use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, DefiniteLength, Entity, Hsla, IntoElement, RenderOnce, SharedString, StyleRefinement,
    Styled, Window, prelude::FluentBuilder as _, relative,
};

use super::{EditorState, Input};
use crate::highlighter::HighlightTheme;
use crate::native_menu::NativeMenu;
use crate::{ActiveTheme as _, RoleOverride, StyledExt as _};

/// A code editor takes its rows from the font, so that a smaller or larger
/// font keeps its leading in proportion.
const EDITOR_LINE_HEIGHT: f32 = 1.5;

/// Colours an application projects onto a code editor, winning over the ones
/// the active theme would supply.
///
/// Every field is optional and only the ones that are set take effect. The
/// rest of the palette — fold icons, diagnostic colours, the caret when it is
/// left unset — keeps coming from the theme, so overriding a palette does not
/// mean reimplementing the control. This is what a host paints with when it
/// wants an editor to follow a palette other than the application theme's,
/// such as a terminal theme: the surfaces read from one source, not two.
#[derive(Clone, Default)]
pub struct EditorStyleOverrides {
    /// Glyph colour for ordinary text.
    pub foreground: Option<Hsla>,
    /// Glyph colour for secondary text: line numbers, folding marks, hints.
    pub muted_foreground: Option<Hsla>,
    /// The editable surface behind the text.
    pub background: Option<Hsla>,
    /// Borders drawn around the editing surface.
    pub border: Option<Hsla>,
    /// Highlight behind selected text.
    pub selection: Option<Hsla>,
    /// The caret. Defaults to the theme's caret, not to `foreground`.
    pub caret: Option<Hsla>,
    /// Syntax colours. A host that has already picked a palette for its
    /// surface supplies one here so tokens and background agree.
    pub highlight_styles: Option<Arc<HighlightTheme>>,
    /// Fill behind the line the caret sits on.
    pub editor_active_line: Option<Hsla>,
    /// Fill behind the line-number gutter.
    ///
    /// Set this whenever `background` differs from the theme's, otherwise the
    /// gutter keeps painting in the theme's colour and the editor reads as two
    /// surfaces stacked side by side.
    pub editor_gutter_background: Option<Hsla>,
    /// Colour for invisible characters.
    pub editor_invisible: Option<Hsla>,
}

impl EditorStyleOverrides {
    /// Project only the fields that were set onto `style`.
    ///
    /// Everything left unset keeps the value `style` already carries, which is
    /// how the component-supplied parts of the look (fold icon renderer,
    /// diagnostics) survive an application palette.
    pub(crate) fn apply_to(&self, style: &mut gpui_base::input::InputEditorStyle) {
        let set = |target: &mut Hsla, value: Option<Hsla>| {
            if let Some(value) = value {
                *target = value;
            }
        };

        set(&mut style.foreground, self.foreground);
        set(&mut style.muted_foreground, self.muted_foreground);
        set(&mut style.background, self.background);
        set(&mut style.border, self.border);
        set(&mut style.selection, self.selection);
        set(&mut style.caret, self.caret);
        if let Some(highlight_styles) = self.highlight_styles.as_ref() {
            style.highlight_styles = highlight_styles.clone();
        }
        if let Some(active_line) = self.editor_active_line {
            style.editor_active_line = Some(active_line);
        }
        if let Some(gutter) = self.editor_gutter_background {
            style.editor_gutter_background = Some(gutter);
        }
        if let Some(invisible) = self.editor_invisible {
            style.editor_invisible = Some(invisible);
        }
    }
}

/// A styled source-code editor.
#[derive(IntoElement)]
pub struct Editor {
    state: Entity<EditorState>,
    style: StyleRefinement,
    height: Option<DefiniteLength>,
    appearance: bool,
    bordered: bool,
    disabled: bool,
    readonly: bool,
    tab_index: isize,
    role: RoleOverride,
    aria_label: Option<SharedString>,

    /// An optional context menu builder to allow a custom context menu.
    ///
    /// If set, this overrides the built-in context menu.
    context_menu_builder: Option<Rc<dyn Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu>>,

    /// An optional palette that wins over the theme-derived one.
    editor_style: Option<EditorStyleOverrides>,
}

impl Editor {
    pub fn new(state: &Entity<EditorState>) -> Self {
        Self {
            state: state.clone(),
            style: StyleRefinement::default(),
            height: None,
            appearance: true,
            bordered: true,
            disabled: false,
            readonly: false,
            tab_index: 0,
            role: RoleOverride::default(),
            aria_label: None,
            context_menu_builder: None,
            editor_style: None,
        }
    }

    pub fn h(mut self, height: impl Into<DefiniteLength>) -> Self {
        self.height = Some(height.into());
        self
    }

    pub fn appearance(mut self, appearance: bool) -> Self {
        self.appearance = appearance;
        self
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the editor to read-only, default is `false`.
    ///
    /// Unlike [`Self::disabled`], a read-only editor keeps the normal appearance
    /// and still can be focused, selected and copied, it only rejects the changes
    /// made by the user.
    pub fn readonly(mut self, readonly: bool) -> Self {
        self.readonly = readonly;
        self
    }

    pub fn tab_index(mut self, index: isize) -> Self {
        self.tab_index = index;
        self
    }

    pub fn role(mut self, role: impl Into<RoleOverride>) -> Self {
        self.role = role.into();
        self
    }

    pub fn aria_label(mut self, label: impl Into<SharedString>) -> Self {
        self.aria_label = Some(label.into());
        self
    }

    /// Replace the built-in context menu shown on right-click.
    ///
    /// The closure receives an empty menu and returns the one to show, so it
    /// decides entirely what appears — the default items are not added.
    pub fn context_menu(
        mut self,
        f: impl Fn(NativeMenu, &mut Window, &mut App) -> NativeMenu + 'static,
    ) -> Self {
        self.context_menu_builder = Some(Rc::new(f));
        self
    }

    /// Paint this editor with a palette of the caller's choosing.
    ///
    /// Only the fields set on `style` win over the active theme; the rest of
    /// the look keeps coming from it. Callers that want the editor to follow,
    /// say, a terminal theme supply the fields that make up a surface —
    /// background, foreground, gutter and active line, plus syntax colours —
    /// and leave the component-owned parts alone.
    pub fn editor_style(mut self, style: EditorStyleOverrides) -> Self {
        self.editor_style = Some(style);
        self
    }
}

impl Styled for Editor {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Editor {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        Input::from_state(self.state.clone())
            // Source code wants a monospace font at a code size, and rows that
            // follow that size. These come first so that a text style set on
            // this editor refines over them: `.text_sm()` and `.font_family()`
            // keep working.
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size)
            .line_height(relative(EDITOR_LINE_HEIGHT))
            .appearance(self.appearance)
            .bordered(self.bordered)
            .focus_bordered(false)
            .disabled(self.disabled)
            .readonly(self.readonly)
            .tab_index(self.tab_index)
            .role(self.role)
            .when_some(self.height, |this, height| this.h(height))
            .when_some(self.aria_label, |this, label| this.aria_label(label))
            .when_some(self.context_menu_builder, |this, build| {
                this.context_menu(move |menu, window, cx| build(menu, window, cx))
            })
            .when_some(self.editor_style, |this, style| this.editor_style(style))
            .refine_style(&self.style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::EditorState;
    use gpui::{
        AppContext as _, Context, ParentElement as _, Pixels, Render, TestAppContext,
        VisualTestContext, div, px,
    };
    use gpui_base::input::{InputEditorStyle, SharedHighlightStyleResolver};

    /// Renders one editor with an optional host palette and returns the style
    /// that actually reached its state.
    fn rendered_editor_style(
        cx: &mut TestAppContext,
        overrides: Option<EditorStyleOverrides>,
    ) -> InputEditorStyle {
        cx.update(crate::init);
        let mut state = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).default_value("fn main() {}"));
            state = Some(editor.clone());
            OverridesHarness {
                state: editor,
                overrides,
            }
        });
        let state = state.unwrap();
        VisualTestContext::update(cx, |window, cx| window.draw(cx).clear(cx));

        cx.read(|cx| state.read(cx).editor_style().clone())
    }

    struct OverridesHarness {
        state: Entity<EditorState>,
        overrides: Option<EditorStyleOverrides>,
    }

    impl Render for OverridesHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let editor = match self.overrides.clone() {
                Some(overrides) => Editor::new(&self.state).editor_style(overrides),
                None => Editor::new(&self.state),
            };

            div().size_full().child(editor)
        }
    }

    #[gpui::test]
    fn a_host_palette_wins_where_it_is_set(cx: &mut TestAppContext) {
        let host_background: Hsla = gpui::rgb(0x0a0e14).into();
        let host_foreground: Hsla = gpui::rgb(0x00d9ff).into();
        let style = rendered_editor_style(
            cx,
            Some(EditorStyleOverrides {
                background: Some(host_background),
                foreground: Some(host_foreground),
                ..Default::default()
            }),
        );
        let theme_border = cx.read(|cx| cx.theme().border);

        assert_eq!(style.background, host_background);
        assert_eq!(style.foreground, host_foreground);
        // Fields the host left alone keep the theme's values, which is what
        // makes a palette override additive rather than a takeover.
        assert_eq!(style.border, theme_border);
    }

    #[gpui::test]
    fn without_a_host_palette_the_editor_follows_the_theme(cx: &mut TestAppContext) {
        let style = rendered_editor_style(cx, None);
        let (background, foreground, gutter) = cx.read(|cx| {
            let theme = cx.theme();
            (
                theme.editor_background(),
                theme.foreground,
                theme.highlight_theme.style.editor_gutter_background,
            )
        });

        assert_eq!(style.background, background);
        assert_eq!(style.foreground, foreground);
        assert_eq!(style.editor_gutter_background, gutter);
    }

    #[test]
    fn an_overlay_touches_only_the_fields_it_sets() {
        let mut painted = InputEditorStyle {
            foreground: gpui::rgb(0x111111).into(),
            muted_foreground: gpui::rgb(0x222222).into(),
            background: gpui::rgb(0x333333).into(),
            border: gpui::rgb(0x444444).into(),
            selection: gpui::rgb(0x555555).into(),
            caret: gpui::rgb(0x666666).into(),
            editor_active_line: Some(gpui::rgb(0x777777).into()),
            editor_gutter_background: Some(gpui::rgb(0x888888).into()),
            editor_invisible: Some(gpui::rgb(0x999999).into()),
            fold_icon_renderer: Some(Rc::new(|_, _| div().into_any_element())),
            ..InputEditorStyle::default()
        };
        let before = painted.clone();

        let host_background: Hsla = gpui::rgb(0x0a0e14).into();
        EditorStyleOverrides {
            background: Some(host_background),
            ..Default::default()
        }
        .apply_to(&mut painted);

        assert_eq!(painted.background, host_background);
        assert_eq!(painted.foreground, before.foreground);
        assert_eq!(painted.muted_foreground, before.muted_foreground);
        assert_eq!(painted.border, before.border);
        assert_eq!(painted.selection, before.selection);
        assert_eq!(painted.caret, before.caret);
        // Moving the surface says nothing about the gutter or the active line:
        // a host that repaints one must repaint the others, which is the seam
        // this whole path exists to keep consistent.
        assert_eq!(painted.editor_active_line, before.editor_active_line);
        assert_eq!(
            painted.editor_gutter_background,
            before.editor_gutter_background
        );
        assert_eq!(painted.editor_invisible, before.editor_invisible);
        // The component's own parts survive a host palette untouched.
        let (painted_fold, before_fold) = (
            painted.fold_icon_renderer.as_ref(),
            before.fold_icon_renderer.as_ref(),
        );
        assert!(matches!((painted_fold, before_fold), (Some(a), Some(b)) if Rc::ptr_eq(a, b)));
    }

    #[test]
    fn a_host_highlight_theme_replaces_the_theme_one() {
        let host_theme = HighlightTheme::default_dark();
        let mut painted = InputEditorStyle::default();
        EditorStyleOverrides {
            highlight_styles: Some(host_theme.clone()),
            ..Default::default()
        }
        .apply_to(&mut painted);

        let expected: SharedHighlightStyleResolver = host_theme;
        assert!(Arc::ptr_eq(&painted.highlight_styles, &expected));
    }

    struct Harness {
        state: Entity<EditorState>,
        /// A text size set on the editor, as `.text_sm()` would.
        text_size: Option<Pixels>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                Editor::new(&self.state)
                    .when_some(self.text_size, |this, size| this.text_size(size)),
            )
        }
    }

    /// The row height the editor laid out with, which follows its font size.
    fn line_height(cx: &mut TestAppContext, text_size: Option<Pixels>) -> Pixels {
        cx.update(crate::init);
        let mut state = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).default_value("fn main() {}"));
            state = Some(editor.clone());
            Harness {
                state: editor,
                text_size,
            }
        });
        let state = state.unwrap();
        VisualTestContext::update(cx, |window, cx| window.draw(cx).clear(cx));

        cx.read(|cx| {
            state
                .read(cx)
                .line_height()
                .expect("the editor must lay out")
        })
    }

    #[gpui::test]
    fn the_rows_follow_the_font_size(cx: &mut TestAppContext) {
        // With nothing set, the theme's monospace size, not the ambient one.
        assert_eq!(line_height(cx, None), px(20.));
        // A text style set on the editor refines over that, rows and all.
        assert_eq!(line_height(cx, Some(px(24.))), px(36.));
        assert_eq!(line_height(cx, Some(px(40.))), px(60.));
    }
    #[gpui::test]
    fn language_config_works_without_render_sync(cx: &mut TestAppContext) {
        use crate::input::{AutoClosingPair, language_config::LanguageConfig, set_language_config};
        use gpui::EntityInputHandler as _;
        cx.update(crate::init);
        let mut state = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).language("plaintext"));
            // Plain-text defaults apply before the first render.
            editor.update(cx, |state, cx| {
                state.replace_text_in_range(None, "(", window, cx);
                assert_eq!(state.text().to_string(), "(");
                state.set_value("", window, cx);
                state.set_highlighter("json", cx);
                state.replace_text_in_range(None, "[", window, cx);
                assert_eq!(state.text().to_string(), "[]");
            });
            state = Some(editor.clone());
            Harness {
                state: editor,
                text_size: None,
            }
        });
        let state = state.unwrap();
        VisualTestContext::update(cx, |_, cx| {
            set_language_config(
                "json",
                LanguageConfig::default().auto_closing_pairs([AutoClosingPair::new("«", "»")]),
                cx,
            );
        });
        // A styled render must preserve the registered configuration.
        VisualTestContext::update(cx, |window, cx| window.draw(cx).clear(cx));
        VisualTestContext::update(cx, |window, cx| {
            state.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.replace_text_in_range(None, "«", window, cx);
                assert_eq!(state.text().to_string(), "«»");
                state.set_value("if enabled:", window, cx);
                state.set_selected_range(11..11, cx);
                state.set_highlighter("python", cx);
                state.focus(window, cx);
            });
        });
        VisualTestContext::update(cx, |window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("enter");
        cx.read(|cx| assert_eq!(state.read(cx).text().to_string(), "if enabled:\n  "));
    }

    #[gpui::test]
    fn aliases_share_config_even_when_registered_before_init(cx: &mut TestAppContext) {
        use crate::input::{AutoClosingPair, language_config::LanguageConfig, set_language_config};
        use gpui::EntityInputHandler as _;
        cx.update(|cx| {
            set_language_config(
                "py",
                LanguageConfig::default().auto_closing_pairs([AutoClosingPair::new("«", "»")]),
                cx,
            )
        });
        cx.update(crate::init);
        cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).language("PYTHON"));
            editor.update(cx, |state, cx| {
                state.replace_text_in_range(None, "«", window, cx);
                assert_eq!(state.text().to_string(), "«»");
            });
            set_language_config("pyi", LanguageConfig::default().auto_closing_pairs([]), cx);
            editor.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.set_highlighter("py", cx);
                state.replace_text_in_range(None, "(", window, cx);
                assert_eq!(state.text().to_string(), "(");
            });
            Harness {
                state: editor,
                text_size: None,
            }
        });
    }

    #[cfg(all(feature = "tree-sitter-python", feature = "tree-sitter-rust"))]
    #[gpui::test]
    fn syntax_follows_language_before_first_render_and_after_switch(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler as _;
        cx.update(crate::init);
        cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| {
                EditorState::new(window, cx)
                    .language("python")
                    .default_value("\"hello world\"")
            });
            editor.update(cx, |state, cx| {
                state.set_selected_range(6..6, cx);
                state.replace_text_in_range(None, "(", window, cx);
                assert_eq!(state.text().to_string(), "\"hello( world\"");
                state.set_value("# comment ", window, cx);
                state.set_selected_range(10..10, cx);
                state.replace_text_in_range(None, "(", window, cx);
                assert_eq!(state.text().to_string(), "# comment (");
                state.set_value("# comment ", window, cx);
                state.set_selected_range(10..10, cx);
                state.set_highlighter("rust", cx);
                state.replace_text_in_range(None, "(", window, cx);
                assert_eq!(state.text().to_string(), "# comment ()");
            });
            Harness {
                state: editor,
                text_size: None,
            }
        });
    }

    #[cfg(feature = "tree-sitter-python")]
    #[gpui::test]
    fn python_pairing_uses_pre_edit_context(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler as _;
        cx.update(crate::init);
        for (before, cursor, typed, expected) in [
            ("x = ", 4, "\"", "x = \"\""),
            ("x = f\"{value}\"", 12, "(", "x = f\"{value()}\""),
            ("x = \"value\"", 7, "(", "x = \"va(lue\""),
            ("x = \"value\"", 5, "(", "x = \"(value\""),
            ("x = \"value\"", 10, "(", "x = \"value(\""),
        ] {
            let mut state = None;
            let (_, cx) = cx.add_window_view(|window, cx| {
                let editor = cx.new(|cx| EditorState::new(window, cx).default_value(before));
                state = Some(editor.clone());
                Harness {
                    state: editor,
                    text_size: None,
                }
            });
            let state = state.unwrap();
            VisualTestContext::update(cx, |window, cx| {
                state.update(cx, |state, cx| {
                    state.set_highlighter("python", cx);
                    state.set_selected_range(cursor..cursor, cx);
                    state.replace_text_in_range(None, typed, window, cx);
                    assert_eq!(state.text().to_string(), expected);
                });
            });
        }
    }
    #[cfg(feature = "tree-sitter-rust")]
    #[gpui::test]
    fn generated_comment_closer_survives_syntax_changes(cx: &mut TestAppContext) {
        use crate::input::{AutoClosingPair, SyntaxContext, language_config::LanguageConfig};
        use gpui::EntityInputHandler as _;
        cx.update(crate::init);
        cx.update(|cx| {
            crate::input::set_language_config(
                "rust",
                LanguageConfig::default().auto_closing_pairs([AutoClosingPair::new("/*", "*/")
                    .not_in([SyntaxContext::String, SyntaxContext::Comment])]),
                cx,
            )
        });
        let mut state = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| EditorState::new(window, cx).language("rust"));
            state = Some(editor.clone());
            Harness {
                state: editor,
                text_size: None,
            }
        });
        let state = state.unwrap();
        VisualTestContext::update(cx, |window, cx| {
            state.update(cx, |state, cx| {
                state.set_highlighter("rust", cx);

                for text in ["/", "*", "x", "*", "/"] {
                    state.replace_text_in_range(None, text, window, cx);
                }
                assert_eq!(state.text().to_string(), "/*x*/");
                state.replace_text_in_range(None, "!", window, cx);
                assert_eq!(state.text().to_string(), "/*x*/!");
            });
        });
    }
}
