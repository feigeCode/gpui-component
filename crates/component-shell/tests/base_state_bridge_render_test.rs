//! A component element (`new Input(state)`) must materialize from a
//! `gpui-base` state handle.
//!
//! The bridge lives in `ShellRuntime::with_component_state`: when the
//! component state store has no entry for a handle, it falls back to the base
//! entity store — the store `InputState.new(...)` mints handles from and the
//! store JS validation consults. Without that fallback every frame logs
//! `failed to materialize 'Input': retained state handle has been released`
//! and the field silently disappears.
//!
//! Why an integration test: `build_error()` and `debug_tree()` describe the
//! *recorded* spec tree; a materialization failure is only a tracing log and
//! a swapped-in fallback element. So this test captures tracing errors while
//! the view renders — that is the only signal that observes materialization.

use std::{
    cell::RefCell,
    ops::Deref as _,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use gpui::{Entity, TestAppContext, VisualTestContext};
use tracing::{
    field::{Field, Visit},
    span::{Attributes, Id, Record},
    subscriber::Interest,
    Event, Metadata, Subscriber,
};

static NEXT_APP: AtomicU64 = AtomicU64::new(0);

struct TempApp(PathBuf);

impl TempApp {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "navop-base-state-bridge-{}-{}",
            std::process::id(),
            NEXT_APP.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).expect("create temporary application directory");
        std::fs::write(path.join("main.js"), source).expect("write temporary application entry");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Collects `error!` events emitted on this thread. The shell reports a
/// component materialization failure as `tracing::error!` — never through
/// `build_error()` — so this is the observable half of materialization.
#[derive(Default)]
struct ErrorCollector {
    errors: Mutex<Vec<String>>,
}

struct MessageVisitor<'a>(&'a mut String);

impl Visit for MessageVisitor<'_> {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0.push_str(&format!("{value:?}"));
        }
    }
}

impl Subscriber for ErrorCollector {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() == &tracing::Level::ERROR
    }

    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        if event.metadata().level() != &tracing::Level::ERROR {
            return;
        }
        let mut message = String::new();
        event.record(&mut MessageVisitor(&mut message));
        if !message.is_empty() {
            self.errors.lock().unwrap().push(message);
        }
    }

    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        if metadata.level() == &tracing::Level::ERROR {
            Interest::always()
        } else {
            Interest::never()
        }
    }
}

struct ScriptRoot(Entity<gpui_shell::ScriptView>);

impl gpui::Render for ScriptRoot {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        self.0.clone()
    }
}

fn mount(
    cx: &mut TestAppContext,
    source: &str,
) -> (VisualTestContext, Entity<gpui_shell::ScriptView>) {
    cx.update(|cx| {
        gpui_component_shell::init(cx);
    });
    let runtime = gpui_component_shell::new_isolated_runtime().expect("runtime");
    let app = TempApp::new(source);
    let loaded = runtime
        .load_application(app.path(), "main.js")
        .expect("load application");
    let mounted = Rc::new(RefCell::new(None));
    let mounted_for_window = mounted.clone();
    let runtime_for_window = runtime.clone();
    let window = cx.add_window(move |window, cx| {
        let view = runtime_for_window
            .mount_application(&loaded, window, cx)
            .expect("mount application");
        *mounted_for_window.borrow_mut() = Some(view.clone());
        ScriptRoot(view)
    });
    let context = VisualTestContext::from_window(*window.deref(), cx);
    let view = mounted.borrow().clone().expect("mounted view");
    (context, view)
}

/// Renders until settled while collecting tracing errors, then returns
/// `(build error, debug tree, captured error logs)`.
fn render_capturing_errors(
    context: &mut VisualTestContext,
    view: &Entity<gpui_shell::ScriptView>,
) -> (Option<String>, String, Vec<String>) {
    let collector = Arc::new(ErrorCollector::default());
    tracing::subscriber::with_default(collector.clone(), || {
        let mut build_error = None;
        let mut tree = String::new();
        for _ in 0..3 {
            context.run_until_parked();
            context.update(|window, cx| window.draw(cx).clear(cx));
        }
        context.update(|_, cx| {
            build_error = view.read(cx).build_error().map(str::to_owned);
            tree = view
                .read(cx)
                .snapshot()
                .expect("snapshot")
                .debug_tree();
        });
        (
            build_error,
            tree,
            collector.errors.lock().unwrap().clone(),
        )
    })
}

/// A component element must materialize from a `gpui-base` state handle, and
/// the state's script API (`value()`/`set_value()`) must keep working — that
/// combination is exactly what a shell page needs from a themed input.
#[gpui::test]
fn component_input_materializes_from_a_gpui_base_state(cx: &mut TestAppContext) {
    let source = r#"
import { View, div } from "gpui-kit";
import { v_flex, InputState, TextareaState } from "gpui-base";
import { Input, Textarea } from "gpui-component";

export default class Bridged extends View {
  init() {
    this.topic = InputState.new({ value: "topic-val", placeholder: "topic" });
    this.body = TextareaState.new({ value: "body-val", rows: 4 });
  }
  render(cx) {
    return v_flex()
      .child(new Input(this.topic).aria_label("probe-topic"))
      .child(new Textarea(this.body).aria_label("probe-body"))
      .child(div().child(`read=${this.topic.value()}|${this.body.value()}`));
  }
}
"#;

    let (mut context, view) = mount(cx, source);
    let (build_error, tree, errors) = render_capturing_errors(&mut context, &view);

    assert_eq!(
        build_error.as_deref(),
        None,
        "the mixed form must build; errors: {errors:?}\ntree:\n{tree}"
    );
    for needle in ["Input :aria_label(registered)", "Textarea :aria_label(registered)"] {
        assert!(
            tree.contains(needle),
            "`{needle}` missing — the element is not the gpui-component one:\n{tree}"
        );
    }
    assert!(
        tree.contains("read=topic-val|body-val"),
        "the gpui-base state must still answer `value()`:\n{tree}"
    );
    assert!(
        errors.iter().all(|error| !error.contains("failed to materialize")),
        "materialization must not fail; captured errors: {errors:?}\ntree:\n{tree}"
    );
    // And specifically not the failure this fix addresses:
    assert!(
        !errors
            .iter()
            .any(|error| error.contains("retained state handle has been released")),
        "the base state handle must be findable at materialization time: {errors:?}"
    );
}
