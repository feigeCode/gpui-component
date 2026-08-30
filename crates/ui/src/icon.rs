use crate::{ActiveTheme, Sizable, Size};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, Hsla, ImageSource, IntoElement, ParentElement,
    Radians, Render, RenderOnce, SharedString, StyleRefinement, Styled, Svg, Transformation,
    Window, div, img, prelude::FluentBuilder as _, svg,
};
use gpui_component_macros::icon_named;
use std::path::PathBuf;

/// Absolute visual sizes for icons, independent from their surrounding text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconSize {
    Micro,
    Small,
    #[default]
    Default,
    Medium,
    Large,
    Display,
    Hero,
}

impl IconSize {
    pub fn pixels(self) -> gpui::Pixels {
        match self {
            Self::Micro => gpui::px(12.),
            Self::Small => gpui::px(14.),
            Self::Default => gpui::px(16.),
            Self::Medium => gpui::px(20.),
            Self::Large => gpui::px(24.),
            Self::Display => gpui::px(32.),
            Self::Hero => gpui::px(40.),
        }
    }
}

impl From<IconSize> for Size {
    fn from(size: IconSize) -> Self {
        Self::Size(size.pixels())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IconColorMode {
    #[default]
    Mono,
    Color,
}

/// Types implementing this trait can automatically be converted to [`Icon`].
///
/// This allows you to implement a custom version of [`IconName`] that functions as a drop-in
/// replacement for other UI components.
pub trait IconNamed {
    /// Returns the embedded path of the icon.
    fn path(self) -> SharedString;

    /// Returns the default color mode inferred from the icon's SVG content.
    fn color_mode(&self) -> IconColorMode {
        IconColorMode::Mono
    }
}

impl<T: IconNamed> From<T> for Icon {
    fn from(value: T) -> Self {
        Icon::build(value)
    }
}

// Generate `IconName` from the icons that `gpui-component-assets` ships.
// The `$VAR` form resolves to the absolute path published by the assets
// crate's `build.rs` (via cargo's `links` mechanism) and re-exported by
// our own `build.rs`. See `gpui_component_macros::icon_named!`'s doc
// comment for the full mechanism.
icon_named!(
    IconName,
    "$GPUI_COMPONENT_DEFAULT_ICONS_DIR",
    [Debug, Copy, PartialEq, Eq, Hash]
);

#[allow(non_upper_case_globals)]
impl IconName {
    // The generated names intentionally follow ordinary PascalCase. Keep the
    // established product acronyms available without hard-coding the registry.
    pub const AI: Self = Self::Ai;
    pub const AILine: Self = Self::AiLine;
    pub const ClickHouseColor: Self = Self::ClickhouseColor;
    pub const ClickHouseLineColor: Self = Self::ClickhouseLineColor;
    pub const DuckDB: Self = Self::Duckdb;
    pub const GitHub: Self = Self::Github;
    pub const MongoDB: Self = Self::Mongodb;
    pub const MongoDBLine: Self = Self::MongodbLine;
    pub const MSSQLColor: Self = Self::MssqlColor;
    pub const MSSQLLineColor: Self = Self::MssqlLineColor;
    pub const MySQLColor: Self = Self::MysqlColor;
    pub const MySQLLineColor: Self = Self::MysqlLineColor;
    pub const OpenEulerColor: Self = Self::OpeneulerColor;
    pub const PostgreSQLColor: Self = Self::PostgresqlColor;
    pub const PostgreSQLLineColor: Self = Self::PostgresqlLineColor;
    pub const SQLiteColor: Self = Self::SqliteColor;
    pub const SQLiteLineColor: Self = Self::SqliteLineColor;

    /// Return the icon as a Entity<Icon>
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        Icon::build(self).view(cx)
    }

    pub fn color(self) -> Icon {
        Icon::build(self).color()
    }

    pub fn mono(self) -> Icon {
        Icon::build(self).mono()
    }
}

impl From<IconName> for AnyElement {
    fn from(val: IconName) -> Self {
        Icon::build(val).into_any_element()
    }
}

impl RenderOnce for IconName {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::build(self)
    }
}

#[derive(IntoElement)]
pub struct Icon {
    base: Svg,
    style: StyleRefinement,
    path: SharedString,
    image_source: Option<ImageSource>,
    text_color: Option<Hsla>,
    size: Option<Size>,
    color_mode: IconColorMode,
    rotation: Option<Radians>,
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            base: svg().flex_none().size_4(),
            style: StyleRefinement::default(),
            path: "".into(),
            image_source: None,
            text_color: None,
            size: None,
            color_mode: IconColorMode::default(),
            rotation: None,
        }
    }
}

impl Clone for Icon {
    fn clone(&self) -> Self {
        let mut this = Self::default().path(self.path.clone());
        this.style = self.style.clone();
        this.rotation = self.rotation;
        this.size = self.size;
        this.text_color = self.text_color;
        this.image_source = self.image_source.clone();
        this.color_mode = self.color_mode;
        this
    }
}

impl Icon {
    pub fn new(icon: impl Into<Icon>) -> Self {
        icon.into()
    }

    fn build(name: impl IconNamed) -> Self {
        let color_mode = name.color_mode();
        Self::default().path(name.path()).color_mode(color_mode)
    }

    /// Set the icon path of the Assets bundle
    ///
    /// For example: `icons/foo.svg`
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self.image_source = None;
        self
    }

    /// Use an image from the filesystem rather than the embedded asset bundle.
    pub fn file_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        self.path = path.display().to_string().into();
        self.image_source = Some(path.into());
        self
    }

    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub(crate) fn path_ref(&self) -> &SharedString {
        &self.path
    }

    /// Create a new view for the icon
    pub fn view(self, cx: &mut App) -> Entity<Icon> {
        cx.new(|_| self)
    }

    pub fn transform(mut self, transformation: gpui::Transformation) -> Self {
        self.base = self.base.with_transformation(transformation);
        self
    }

    pub fn empty() -> Self {
        Self::default()
    }

    pub fn color_mode(mut self, mode: IconColorMode) -> Self {
        self.color_mode = mode;
        self
    }

    pub fn color(mut self) -> Self {
        self.color_mode = IconColorMode::Color;
        self
    }

    pub fn mono(mut self) -> Self {
        self.color_mode = IconColorMode::Mono;
        self
    }

    /// Rotate the icon by the given angle
    pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
        self.base = self
            .base
            .with_transformation(Transformation::rotate(radians));
        self
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }

    fn text_color(mut self, color: impl palette::IntoColor<Hsla>) -> Self {
        self.text_color = Some(color.into_color());
        self
    }
}

impl Sizable for Icon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.size = Some(size.into());
        self
    }
}

/// A monochrome action or control icon.
#[derive(IntoElement, Clone)]
pub struct FunctionalIcon {
    icon: Icon,
}

impl FunctionalIcon {
    pub fn new(name: IconName) -> Self {
        Self {
            icon: Icon::new(name).mono(),
        }
    }

    pub fn text_color(mut self, color: impl palette::IntoColor<Hsla>) -> Self {
        self.icon = self.icon.text_color(color);
        self
    }

    pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
        self.icon = self.icon.rotate(radians);
        self
    }

    pub fn transform(mut self, transformation: Transformation) -> Self {
        self.icon = self.icon.transform(transformation);
        self
    }

    pub fn into_icon(self) -> Icon {
        self.icon
    }
}

impl Styled for FunctionalIcon {
    fn style(&mut self) -> &mut StyleRefinement {
        self.icon.style()
    }

    fn text_color(self, color: impl palette::IntoColor<Hsla>) -> Self {
        FunctionalIcon::text_color(self, color)
    }
}

impl Sizable for FunctionalIcon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.icon = self.icon.with_size(size);
        self
    }
}

impl From<FunctionalIcon> for Icon {
    fn from(icon: FunctionalIcon) -> Self {
        icon.into_icon()
    }
}

impl RenderOnce for FunctionalIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.into_icon()
    }
}

/// An original-color icon used for product, platform, or database identity.
#[derive(IntoElement, Clone)]
pub struct BrandIcon {
    icon: Icon,
}

impl BrandIcon {
    pub fn new(name: IconName) -> Self {
        Self {
            icon: Icon::new(name).color(),
        }
    }

    pub fn into_icon(self) -> Icon {
        self.icon
    }
}

impl Sizable for BrandIcon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.icon = self.icon.with_size(size);
        self
    }
}

impl From<BrandIcon> for Icon {
    fn from(icon: BrandIcon) -> Self {
        icon.into_icon()
    }
}

impl RenderOnce for BrandIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.into_icon()
    }
}

/// A monochrome icon representing an application or domain object.
#[derive(IntoElement, Clone)]
pub struct ObjectIcon {
    icon: Icon,
}

impl ObjectIcon {
    pub fn new(name: IconName) -> Self {
        Self {
            icon: Icon::new(name).mono(),
        }
    }

    pub fn text_color(mut self, color: impl palette::IntoColor<Hsla>) -> Self {
        self.icon = self.icon.text_color(color);
        self
    }

    pub fn into_icon(self) -> Icon {
        self.icon
    }
}

impl Styled for ObjectIcon {
    fn style(&mut self) -> &mut StyleRefinement {
        self.icon.style()
    }

    fn text_color(self, color: impl palette::IntoColor<Hsla>) -> Self {
        ObjectIcon::text_color(self, color)
    }
}

impl Sizable for ObjectIcon {
    fn with_size(mut self, size: impl Into<Size>) -> Self {
        self.icon = self.icon.with_size(size);
        self
    }
}

impl From<ObjectIcon> for Icon {
    fn from(icon: ObjectIcon) -> Self {
        icon.into_icon()
    }
}

impl RenderOnce for ObjectIcon {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.into_icon()
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());
        let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

        match self.color_mode {
            IconColorMode::Mono => {
                let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
                let mut base = self.base;
                *base.style() = self.style;

                base.flex_shrink_0()
                    .text_color(text_color)
                    .when(!has_base_size, |this| this.size(text_size))
                    .when_some(self.size, apply_icon_size)
                    .path(self.path)
                    .into_any_element()
            }
            IconColorMode::Color => {
                let mut base = div();
                *base.style() = self.style;

                base.flex_shrink_0()
                    .when(!has_base_size, |this| this.size(text_size))
                    .when_some(self.size, apply_icon_size)
                    .child(img(self.image_source.unwrap_or_else(|| self.path.into())).size_full())
                    .into_any_element()
            }
        }
    }
}

impl From<Icon> for AnyElement {
    fn from(val: Icon) -> Self {
        val.into_any_element()
    }
}

impl Render for Icon {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let text_size = window.text_style().font_size.to_pixels(window.rem_size());
        let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

        match self.color_mode {
            IconColorMode::Mono => {
                let text_color = self.text_color.unwrap_or_else(|| cx.theme().foreground);
                let mut base = svg().flex_none();
                *base.style() = self.style.clone();

                base.flex_shrink_0()
                    .text_color(text_color)
                    .when(!has_base_size, |this| this.size(text_size))
                    .when_some(self.size, apply_icon_size)
                    .path(self.path.clone())
                    .when_some(self.rotation, |this, rotation| {
                        this.with_transformation(Transformation::rotate(rotation))
                    })
                    .into_any_element()
            }
            IconColorMode::Color => {
                let mut base = div();
                *base.style() = self.style.clone();

                base.flex_shrink_0()
                    .when(!has_base_size, |this| this.size(text_size))
                    .when_some(self.size, apply_icon_size)
                    .child(
                        img(self
                            .image_source
                            .clone()
                            .unwrap_or_else(|| self.path.clone().into()))
                        .size_full(),
                    )
                    .into_any_element()
            }
        }
    }
}

fn apply_icon_size<T: Styled>(this: T, size: Size) -> T {
    match size {
        Size::Size(px) => this.size(px),
        Size::XSmall => this.size_3(),
        Size::Small => this.size_3p5(),
        Size::Medium => this.size_4(),
        Size::Large => this.size_6(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_icons_use_svg_content_for_the_default_mode() {
        for icon in [
            IconName::MongoDB,
            IconName::Redis,
            IconName::Database,
            IconName::Terminal,
            IconName::Vnc,
            IconName::Procedure,
            IconName::FolderFunctions,
            IconName::StatusConnectedLocked,
        ] {
            assert_eq!(Icon::new(icon).color_mode, IconColorMode::Color);
        }
        assert_eq!(Icon::new(IconName::RdpLine).color_mode, IconColorMode::Mono);
        assert_eq!(Icon::new(IconName::VncLine).color_mode, IconColorMode::Mono);
        assert_eq!(Icon::new(IconName::Monitor).color_mode, IconColorMode::Mono);
        assert_eq!(Icon::new(IconName::Paste).color_mode, IconColorMode::Mono);
    }

    #[test]
    fn explicit_color_modes_override_the_generated_default() {
        assert_eq!(
            Icon::new(IconName::RdpLine).color().color_mode,
            IconColorMode::Color
        );
        assert_eq!(
            Icon::new(IconName::MongoDB).mono().color_mode,
            IconColorMode::Mono
        );
    }
}
