//! # Translation Overlay Widget
//!
//! Full-window content widget displayed in the translation overlay window.
//! Shows a scrolling list of completed sentences (source + translation) and
//! the current in-progress ASR text at the bottom.
//!
//! ## Layout
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │  [source 1 - small, gray]                    │  ↑
//! │  Translation 1 - large, white                │  │ scroll
//! │                                              │  │
//! │  [source 2 - small, gray]                    │  │
//! │  Translation 2 - large, white                │  │
//! │                                              │  │
//! │  [pending ASR text - small, amber]           │  ↓
//! │  (bottom_spacer — dynamic, for anchor)       │
//! ├──────────────────────────────────────────────┤
//! │              Hen Local Translator - ...          │  footer (font configurable)
//! └──────────────────────────────────────────────┘
//! ```
//!
//! ## Scroll anchor
//!
//! `bottom_spacer` height and pending_label margin (60px for translation placeholder)
//! create an anchor effect: when scrolled to bottom, the last sentence appears at
//! ~50% of viewport height.
//!
//! Behavior rules:
//! 1) In-progress text (ASR/translating) and completed text share the same
//!    scroll behavior.
//! 2) If content is still short, keep it naturally top-aligned (no forced center).
//! 3) Only when content grows enough do we auto-scroll so the latest line stays
//!    near the vertical center.

use makepad_widgets::*;

live_design! {
    use link::theme::*;
    use link::shaders::*;
    use link::widgets::*;

    use crate::theme::FONT_REGULAR;
    use crate::theme::WHITE;
    use crate::theme::MOXIN_BG_PRIMARY_DARK;
    use crate::theme::MOXIN_TEXT_MUTED_DARK;

    TranslationHistory = {{TranslationHistory}} {
        width: Fill, height: Fit
        flow: Right { wrap: true }
        padding: 0.0
        font_size: 24.0
        font_color: (WHITE)
        draw_normal: {
            color: (WHITE)
            text_style: <FONT_REGULAR> { font_size: 24.0 }
        }
    }

    // Clipped caption pane scroller. The scroll bar must stay logically enabled
    // (set_scroll_pos is a no-op when show_scroll_y is false) but is drawn
    // fully transparent — scrolling is driven by the smooth-scroll animation.
    CaptionScroll = <ScrollYView> {
        width: Fill, height: Fill
        flow: Down
        scroll_bars: <ScrollBars> {
            show_scroll_x: false
            show_scroll_y: true
            scroll_bar_y: {
                draw_bg: {
                    fn pixel(self) -> vec4 {
                        return vec4(0.0, 0.0, 0.0, 0.0);
                    }
                }
            }
        }
    }

    // Small language tag shown at the top of each caption pane.
    LangChip = <RoundedView> {
        width: Fit, height: Fit
        padding: { left: 10, right: 10, top: 3, bottom: 3 }
        draw_bg: {
            instance border_radius: 9.0
            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                sdf.box(0., 0., self.rect_size.x, self.rect_size.y, self.border_radius);
                sdf.fill(vec4(1.0, 1.0, 1.0, 0.08));
                sdf.stroke(vec4(1.0, 1.0, 1.0, 0.14), 1.0);
                return sdf.result;
            }
        }
        chip_label = <Label> {
            width: Fit, height: Fit
            draw_text: {
                color: vec4(0.72, 0.76, 0.82, 1.0)
                text_style: <FONT_REGULAR> { font_size: 11.0 }
            }
            text: "EN"
        }
    }

    pub TranslationOverlay = {{TranslationOverlay}} {
        width: Fill, height: Fill
        flow: Down
        show_bg: true
        draw_bg: {
            instance bg_opacity: 1.0
            fn pixel(self) -> vec4 {
                let base = (MOXIN_BG_PRIMARY_DARK);
                return vec4(base.x, base.y, base.z, self.bg_opacity);
            }
        }

        // ── Dual-language caption view (default) ────────────────────────────
        // Two separated language panes like a conference caption screen:
        // translation on top, source below, each smooth-scrolling as text grows.
        split_view = <View> {
            visible: true
            width: Fill, height: Fill
            flow: Down
            padding: { left: 22, right: 22, top: 14, bottom: 6 }

            translation_pane = <View> {
                width: Fill, height: Fill
                flow: Down
                spacing: 8

                translation_chip = <LangChip> {}

                translation_scroll = <CaptionScroll> {
                    translation_text = <Label> {
                        width: Fill, height: Fit
                        padding: 0.0
                        draw_text: {
                            color: (WHITE)
                            text_style: <FONT_REGULAR> { font_size: 24.0, line_spacing: 1.35 }
                            wrap: Word
                        }
                        text: ""
                    }
                }
            }

            split_divider = <View> {
                width: Fill, height: 1
                margin: { top: 12, bottom: 12 }
                show_bg: true
                draw_bg: {
                    fn pixel(self) -> vec4 {
                        // Hairline that fades out toward both edges.
                        let fade = sin(3.14159 * self.pos.x);
                        return vec4(1.0, 1.0, 1.0, 0.18 * fade);
                    }
                }
            }

            source_pane = <View> {
                width: Fill, height: Fill
                flow: Down
                spacing: 8

                source_chip = <LangChip> {}

                source_scroll = <CaptionScroll> {
                    source_text_label = <Label> {
                        width: Fill, height: Fit
                        padding: 0.0
                        draw_text: {
                            color: vec4(0.80, 0.84, 0.90, 1.0)
                            text_style: <FONT_REGULAR> { font_size: 23.0, line_spacing: 1.35 }
                            wrap: Word
                        }
                        text: ""
                    }
                }
            }
        }

        // ── Classic interleaved sentence list (kept as fallback) ─────────────
        // bottom_spacer height is set dynamically in set_viewport_height() so the
        // last sentence anchors at ~50% of the viewport regardless of window size.
        content_scroll = <ScrollYView> {
            visible: false
            width: Fill, height: Fill
            flow: Down
            align: { x: 0.0, y: 0.0 }
            padding: { left: 16, right: 16, top: 12, bottom: 0 }

            // history_flow: completed sentences with gray source and white translation.
            history_flow = <TranslationHistory> {
                width: Fill, height: Fit
            }

            // pending_label: current ASR text (not yet translated)
            pending_label = <Label> {
                width: Fill, height: Fit
                margin: { top: 8, bottom: 8 }
                align: { x: 0.0, y: 0.0 }
                padding: 0.0
                draw_text: {
                    color: (MOXIN_TEXT_MUTED_DARK)
                    text_style: <FONT_REGULAR> { font_size: 23.0 }
                    wrap: Word
                }
                text: ""
            }

            // Dynamic spacer used only when content is long enough to require
            // centering the newest line; otherwise stays zero.
            bottom_spacer = <View> { width: Fill, height: 0.0 }
        }

        // ── Bottom branding footer ────────────────────────────────────────────
        overlay_footer = <View> {
            width: Fill, height: Fit
            flow: Right
            align: {x: 0.5, y: 0.5}
            spacing: 5
            padding: {left: 10, right: 10, top: 4, bottom: 4}

            footer_left_spacer = <View> { width: 58, height: 1 }

            footer_brand = <View> {
                width: Fill, height: Fit
                flow: Right
                align: {x: 0.5, y: 0.5}
                spacing: 5

                footer_logo = <Image> {
                    width: 22, height: 22
                    source: dep("crate://self/resources/hen_local_icon.png")
                    fit: Smallest
                }

                footer_label = <Label> {
                    width: Fit
                    draw_text: {
                        color: (MOXIN_TEXT_MUTED_DARK)
                        text_style: <FONT_REGULAR> { font_size: 10.0 }
                    }
                    text: "Hen Local Translator - Fully offline live translation, private by design"
                }
            }

            footer_controls = <View> {
                width: 58, height: Fit
                flow: Right
                align: {x: 1.0, y: 0.5}
                spacing: 8

                overlay_stop_btn = <Button> {
                    width: 22, height: 22
                    visible: false
                    text: "■"
                    padding: 0
                    draw_bg: {
                        instance hover: 0.0
                        instance pressed: 0.0
                        instance border_radius: 11.0
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.circle(self.rect_size.x * 0.5, self.rect_size.y * 0.5, self.rect_size.x * 0.5);
                            let base = vec4(0.85, 0.25, 0.25, 0.92);
                            let hover = vec4(0.95, 0.32, 0.32, 1.0);
                            sdf.fill(mix(base, hover, self.hover));
                            return sdf.result;
                        }
                    }
                    draw_text: {
                        text_style: <FONT_REGULAR> { font_size: 9.0 }
                        fn get_color(self) -> vec4 { return vec4(1.0, 1.0, 1.0, 1.0); }
                    }
                }

                overlay_status_dot = <View> {
                    width: 12, height: 12
                    show_bg: true
                    draw_bg: {
                        instance dot_color: vec4(0.451, 0.463, 0.478, 1.0)
                        fn pixel(self) -> vec4 {
                            let sdf = Sdf2d::viewport(self.pos * self.rect_size);
                            sdf.circle(self.rect_size.x * 0.5, self.rect_size.y * 0.5, 5.5);
                            sdf.fill(self.dot_color);
                            return sdf.result;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum HistoryTone {
    Source,
    Translation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HistorySegment {
    text: String,
    tone: HistoryTone,
}

impl HistorySegment {
    fn source(text: &str) -> Self {
        Self {
            text: text.to_string(),
            tone: HistoryTone::Source,
        }
    }

    fn translation(text: &str) -> Self {
        Self {
            text: text.to_string(),
            tone: HistoryTone::Translation,
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TranslationHistory {
    #[deref]
    text_flow: TextFlow,
    #[rust]
    segments: Vec<HistorySegment>,
    #[rust(24.0)]
    translation_font_size: f32,
    #[rust(23.0)]
    source_font_size: f32,
}

impl Widget for TranslationHistory {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.text_flow.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.text_flow.begin(cx, walk);
        self.draw_segments(cx);
        self.text_flow.end(cx);
        DrawStep::done()
    }
}

impl TranslationHistory {
    const SOURCE_COLOR: Vec4f = Vec4f {
        x: 0.451,
        y: 0.463,
        z: 0.478,
        w: 1.0,
    };
    const TRANSLATION_COLOR: Vec4f = Vec4f {
        x: 1.0,
        y: 1.0,
        z: 1.0,
        w: 1.0,
    };

    fn draw_segments(&mut self, cx: &mut Cx2d) {
        let len = self.segments.len();
        for (idx, segment) in self.segments.iter().enumerate() {
            match segment.tone {
                HistoryTone::Source => {
                    self.text_flow.font_colors.push(Self::SOURCE_COLOR);
                    self.text_flow.font_sizes.push(self.source_font_size);
                    self.text_flow.draw_text(cx, &segment.text);
                    self.text_flow.font_sizes.pop();
                    self.text_flow.font_colors.pop();
                    self.text_flow.new_line_collapsed(cx);
                }
                HistoryTone::Translation => {
                    self.text_flow.font_colors.push(Self::TRANSLATION_COLOR);
                    self.text_flow.font_sizes.push(self.translation_font_size);
                    self.text_flow.draw_text(cx, &segment.text);
                    self.text_flow.font_sizes.pop();
                    self.text_flow.font_colors.pop();
                    if idx + 1 < len {
                        self.text_flow.new_line_collapsed_with_spacing(
                            cx,
                            self.translation_font_size as f64 * 0.6,
                        );
                    }
                }
            }
        }
    }

    fn set_segments(&mut self, cx: &mut Cx, segments: Vec<HistorySegment>) {
        if self.segments != segments {
            self.segments = segments;
            self.text_flow.redraw(cx);
        }
    }

    fn set_font_sizes(&mut self, cx: &mut Cx, translation_size: f64, source_size: f64) {
        let translation_size = translation_size as f32;
        let source_size = source_size as f32;
        if (self.translation_font_size - translation_size).abs() < f32::EPSILON
            && (self.source_font_size - source_size).abs() < f32::EPSILON
        {
            return;
        }
        self.translation_font_size = translation_size;
        self.source_font_size = source_size;
        self.text_flow.redraw(cx);
    }
}

impl TranslationHistoryRef {
    fn set_segments(&self, cx: &mut Cx, segments: Vec<HistorySegment>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_segments(cx, segments);
        }
    }

    fn set_font_sizes(&self, cx: &mut Cx, translation_size: f64, source_size: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_font_sizes(cx, translation_size, source_size);
        }
    }
}

#[derive(Live, LiveHook, Widget)]
pub struct TranslationOverlay {
    #[deref]
    view: View,

    /// Font size preset id for content text: "16" | "20" | ... | "160".
    #[rust]
    font_size_preset: String,

    /// Font size preset id for the bottom branding label.
    #[rust]
    footer_font_size_preset: String,

    /// Anchor position preset percentage: "35" | "50" | "70" | "100".
    #[rust]
    anchor_position_preset: String,

    /// Cached history length for detecting changes
    #[rust]
    last_history_len: usize,

    /// Cached pending text for detecting changes
    #[rust]
    last_pending_text: String,

    /// True when content changed and we need to scroll to bottom on next draw.
    #[rust]
    pending_scroll: bool,

    /// Viewport height hint (window height minus footer) set by shell.
    /// The widget prefers measuring real scroll-view height during draw and
    /// falls back to this value when area data is unavailable.
    #[rust]
    viewport_height: f64,

    /// Last applied bottom spacer height, to avoid redundant apply_over calls.
    #[rust]
    last_spacer_height: f64,

    /// True when there is in-progress text shown in pending_label.
    #[rust]
    pending_active: bool,

    /// Whether content exceeds half viewport and should follow tail scrolling.
    #[rust]
    follow_tail_scroll: bool,

    /// True when only pending text is present (no completed history yet).
    #[rust]
    pending_only_mode: bool,

    /// Passthrough mode — no translation, source text IS the output.
    #[rust]
    passthrough: bool,

    /// Language used for the idle sample text shown before translation starts.
    #[rust]
    placeholder_lang: String,

    /// True when the overlay is showing its idle sample rather than runtime text.
    #[rust]
    idle_placeholder_visible: bool,

    #[rust(true)]
    locale_en: bool,

    #[rust]
    status: String,

    /// Dual-language split caption view (translation pane + source pane).
    #[rust(true)]
    split_mode: bool,

    /// Language codes from the active pair, for the pane chips.
    #[rust]
    source_lang_code: String,
    #[rust]
    target_lang_code: String,

    /// Smooth-scroll state per pane: current animated offset and target.
    #[rust]
    trans_scroll_cur: f64,
    #[rust]
    trans_scroll_target: f64,
    #[rust]
    src_scroll_cur: f64,
    #[rust]
    src_scroll_target: f64,

    /// Frame driver for the smooth-scroll animation.
    #[rust]
    anim_frame: NextFrame,

    /// Cached split-view texts to avoid redundant relayouts.
    #[rust]
    last_split_translation: String,
    #[rust]
    last_split_source: String,
}

impl Widget for TranslationOverlay {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.anim_frame.is_event(event).is_some() {
            self.step_smooth_scroll(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        // ── Draw content first ────────────────────────────────────────────────
        //
        // IMPORTANT: draw_walk must complete before set_scroll_pos is called.
        // Reason: scroll_bar.set_scroll_pos() clamps immediately to
        //   min(value, view_total - view_visible)
        // where view_total is updated only during draw_scroll_bars() which runs
        // at the end of view.draw_walk(). If we set scroll before draw, view_total
        // is stale (previous frame) and f64::MAX clamps to old max.
        //
        // After draw_walk, view_total reflects the freshly laid-out content, so
        // f64::MAX clamps to the correct new maximum scroll position.

        let result = self.view.draw_walk(cx, scope, walk);

        if self.split_mode {
            // Measure fresh layout and retarget the smooth-scroll animation.
            self.update_split_scroll_targets(cx);
            return result;
        }

        // Keep last sentence vertically centered while compensating for dynamic
        // pending label height (wrap differs between compact/fullscreen widths).
        self.update_anchor_spacer_from_layout(cx);

        // ── Set scroll after draw (view_total is now current) ─────────────────
        if self.pending_only_mode {
            // First pending ASR line must stay in natural top-flow.
            self.view
                .view(ids!(content_scroll))
                .set_scroll_pos(cx, dvec2(0.0, 0.0));
        } else if self.pending_scroll {
            self.pending_scroll = false;
            let target_y = if self.follow_tail_scroll {
                f64::MAX
            } else {
                0.0
            };
            self.view
                .view(ids!(content_scroll))
                .set_scroll_pos(cx, dvec2(0.0, target_y));
            // One more redraw to render with the updated scroll position.
            self.view.redraw(cx);
        }

        result
    }
}

impl TranslationOverlay {
    const SCROLL_PADDING_TOP: f64 = 12.0;
    const PENDING_MARGIN_TOP: f64 = 8.0;
    const PENDING_MARGIN_BOTTOM: f64 = 8.0;
    const TAIL_SAFE_GAP: f64 = 10.0;

    /// Per-frame easing factor for smooth caption scrolling (0..1).
    const SMOOTH_SCROLL_EASE: f64 = 0.16;
    /// Snap threshold: below this distance the animation lands on target.
    const SMOOTH_SCROLL_SNAP: f64 = 0.5;
    /// Keep this many recent sentences per pane.
    const SPLIT_KEEP_SENTENCES: usize = 16;

    fn lang_display(code: &str) -> &'static str {
        match code {
            "zh" => "中文",
            "en" => "EN",
            "ja" => "日本語",
            "ko" => "한국어",
            "fr" => "FR",
            "de" => "DE",
            "es" => "ES",
            "ru" => "RU",
            _ => "•",
        }
    }

    /// Join sentences one-per-line so each committed sentence reads as its own
    /// caption row instead of one dense run-on paragraph.
    fn join_sentences<'a>(parts: impl Iterator<Item = &'a str>) -> String {
        let mut out = String::new();
        for part in parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(part);
        }
        out
    }

    /// Build the continuous (translation, source) texts for the split panes.
    fn build_split_texts(
        history: &[(String, String)],
        pending: &str,
        passthrough: bool,
    ) -> (String, String) {
        let start = history.len().saturating_sub(Self::SPLIT_KEEP_SENTENCES);
        let recent = &history[start..];
        if passthrough {
            // Single language: everything flows into the primary pane.
            let text = Self::join_sentences(
                recent
                    .iter()
                    .map(|(source, _)| source.as_str())
                    .chain(std::iter::once(pending)),
            );
            return (text, String::new());
        }
        let translation = Self::join_sentences(
            recent
                .iter()
                .map(|(_, translation)| translation.as_str()),
        );
        let source = Self::join_sentences(
            recent
                .iter()
                .map(|(source, _)| source.as_str())
                .chain(std::iter::once(pending)),
        );
        (translation, source)
    }

    /// Apply new split-view texts and chip labels, and kick the scroll animation.
    fn apply_split_update(&mut self, cx: &mut Cx, translation: String, source: String) {
        if translation == self.last_split_translation && source == self.last_split_source {
            return;
        }
        self.last_split_translation = translation.clone();
        self.last_split_source = source.clone();
        self.view
            .label(ids!(split_view.translation_pane.translation_scroll.translation_text))
            .set_text(cx, &translation);
        self.view
            .label(ids!(split_view.source_pane.source_scroll.source_text_label))
            .set_text(cx, &source);
        self.view.redraw(cx);
    }

    fn update_split_chips(&mut self, cx: &mut Cx) {
        let (top_code, show_source) = if self.passthrough {
            (self.source_lang_code.as_str(), false)
        } else {
            (self.target_lang_code.as_str(), true)
        };
        self.view
            .label(ids!(split_view.translation_pane.translation_chip.chip_label))
            .set_text(cx, Self::lang_display(top_code));
        self.view
            .label(ids!(split_view.source_pane.source_chip.chip_label))
            .set_text(cx, Self::lang_display(&self.source_lang_code));
        self.view
            .view(ids!(split_view.source_pane))
            .set_visible(cx, show_source);
        self.view
            .view(ids!(split_view.split_divider))
            .set_visible(cx, show_source);
    }

    /// Measure pane layouts after a draw pass and update scroll targets. Runs
    /// inside draw_walk, so all areas reflect the freshly laid-out content.
    fn update_split_scroll_targets(&mut self, cx: &mut Cx2d) {
        let measure = |view: ViewRef, label: LabelRef| -> f64 {
            let viewport_h = view.area().rect(cx).size.y.max(0.0);
            let content_h = label.area().rect(cx).size.y.max(0.0);
            (content_h + Self::TAIL_SAFE_GAP - viewport_h).max(0.0)
        };
        let trans_target = measure(
            self.view
                .view(ids!(split_view.translation_pane.translation_scroll)),
            self.view
                .label(ids!(split_view.translation_pane.translation_scroll.translation_text)),
        );
        let src_target = measure(
            self.view.view(ids!(split_view.source_pane.source_scroll)),
            self.view
                .label(ids!(split_view.source_pane.source_scroll.source_text_label)),
        );
        self.trans_scroll_target = trans_target;
        self.src_scroll_target = src_target;
        if (self.trans_scroll_target - self.trans_scroll_cur).abs() > Self::SMOOTH_SCROLL_SNAP
            || (self.src_scroll_target - self.src_scroll_cur).abs() > Self::SMOOTH_SCROLL_SNAP
        {
            self.anim_frame = cx.new_next_frame();
        }
    }

    /// One animation frame: ease each pane's offset toward its target.
    fn step_smooth_scroll(&mut self, cx: &mut Cx) {
        let mut animating = false;
        for (cur, target) in [
            (&mut self.trans_scroll_cur, self.trans_scroll_target),
            (&mut self.src_scroll_cur, self.src_scroll_target),
        ] {
            let delta = target - *cur;
            if delta.abs() > Self::SMOOTH_SCROLL_SNAP {
                *cur += delta * Self::SMOOTH_SCROLL_EASE;
                animating = true;
            } else {
                *cur = target;
            }
        }
        self.view
            .view(ids!(split_view.translation_pane.translation_scroll))
            .set_scroll_pos(cx, dvec2(0.0, self.trans_scroll_cur));
        self.view
            .view(ids!(split_view.source_pane.source_scroll))
            .set_scroll_pos(cx, dvec2(0.0, self.src_scroll_cur));
        if animating {
            self.anim_frame = cx.new_next_frame();
        }
        self.view.redraw(cx);
    }

    const FONT_SIZE_PRESETS: &'static [&'static str] = &[
        "16", "20", "24", "30", "36", "44", "52", "64", "80", "96", "120", "160",
    ];

    const FOOTER_FONT_SIZE_PRESETS: &'static [&'static str] = &[
        "8", "10", "12", "14", "16", "18", "20", "22", "24", "26", "28", "30", "32",
    ];

    fn font_size_preset_values(preset: &str) -> (f64, f64) {
        let size: f64 = preset.parse().unwrap_or(24.0);
        (size, (size - 1.0).max(8.0))
    }

    fn footer_font_size_value(preset: &str) -> f64 {
        preset.parse().unwrap_or(20.0)
    }

    fn footer_logo_size_value(preset: &str) -> f64 {
        Self::footer_font_size_value(preset).max(22.0)
    }

    fn anchor_position_ratio(preset: &str) -> f64 {
        match preset {
            "35" => 0.35,
            "50" => 0.5,
            "70" => 0.7,
            "100" => 1.0,
            _ => 0.5,
        }
    }

    fn update_font_size_draw_styles(&self, cx: &mut Cx) {
        let (history_size, pending_size) = Self::font_size_preset_values(&self.font_size_preset);
        self.view
            .translation_history(ids!(content_scroll.history_flow))
            .set_font_sizes(cx, history_size, pending_size);
        self.view
            .label(ids!(content_scroll.pending_label))
            .apply_over(
                cx,
                live! { draw_text: { text_style: { font_size: (pending_size) } } },
            );
        self.view
            .label(ids!(split_view.translation_pane.translation_scroll.translation_text))
            .apply_over(
                cx,
                live! { draw_text: { text_style: { font_size: (history_size) } } },
            );
        self.view
            .label(ids!(split_view.source_pane.source_scroll.source_text_label))
            .apply_over(
                cx,
                live! { draw_text: { text_style: { font_size: (pending_size) } } },
            );
    }

    fn update_footer_font_size_draw_styles(&self, cx: &mut Cx) {
        let size = Self::footer_font_size_value(&self.footer_font_size_preset);
        let logo_size = Self::footer_logo_size_value(&self.footer_font_size_preset);
        self.view
            .label(ids!(overlay_footer.footer_brand.footer_label))
            .apply_over(
                cx,
                live! { draw_text: { text_style: { font_size: (size) } } },
            );
        self.view
            .image(ids!(overlay_footer.footer_brand.footer_logo))
            .apply_over(cx, live! { width: (logo_size), height: (logo_size) });
    }

    fn footer_brand_text(locale_en: bool) -> &'static str {
        if locale_en {
            "Hen Local Translator - Fully offline live translation, private by design"
        } else {
            "很Local 实时翻译 - 完全离线本地部署，隐私优先"
        }
    }

    fn idle_placeholder_text(lang: &str) -> &'static str {
        match lang {
            "zh" => "本地 AI 正在待命，字幕和声音只留在你的设备上。",
            "ja" => "ローカルAIが待機中です。字幕と音声は端末内に留まります。",
            "fr" => "L'IA locale est prête. Voix et sous-titres restent sur cet appareil.",
            _ => "Local AI is ready. Subtitles and voice stay on this device.",
        }
    }

    fn has_runtime_content(&self) -> bool {
        self.last_history_len > 0 || self.pending_active || !self.last_pending_text.is_empty()
    }

    fn show_idle_placeholder_if_empty(&mut self, cx: &mut Cx) {
        if self.has_runtime_content() {
            return;
        }
        if self.split_mode {
            self.idle_placeholder_visible = true;
            self.last_split_translation.clear();
            self.last_split_source.clear();
            self.view
                .label(ids!(split_view.translation_pane.translation_scroll.translation_text))
                .set_text(cx, Self::idle_placeholder_text(&self.placeholder_lang));
            self.view
                .label(ids!(split_view.source_pane.source_scroll.source_text_label))
                .set_text(cx, "");
            self.view.redraw(cx);
            return;
        }
        self.idle_placeholder_visible = true;
        self.pending_active = false;
        self.follow_tail_scroll = false;
        self.pending_only_mode = false;
        self.pending_scroll = true;
        self.last_spacer_height = -1.0;
        self.view
            .translation_history(ids!(content_scroll.history_flow))
            .set_segments(
                cx,
                vec![HistorySegment::translation(Self::idle_placeholder_text(
                    &self.placeholder_lang,
                ))],
            );
        let pending_label = self.view.label(ids!(content_scroll.pending_label));
        pending_label.set_text(cx, "");
        pending_label.apply_over(cx, live! { margin: { top: 0.0, bottom: 0.0 } });
        self.view.redraw(cx);
    }

    pub fn set_locale(&mut self, cx: &mut Cx, locale_en: bool) {
        if self.locale_en == locale_en {
            return;
        }
        self.locale_en = locale_en;
        self.view
            .label(ids!(overlay_footer.footer_brand.footer_label))
            .set_text(cx, Self::footer_brand_text(locale_en));
        if self.idle_placeholder_visible {
            self.show_idle_placeholder_if_empty(cx);
        }
        self.view.redraw(cx);
    }

    pub fn set_status(&mut self, cx: &mut Cx, status: &str) {
        if self.status == status {
            if status == "idle" && !self.idle_placeholder_visible {
                self.show_idle_placeholder_if_empty(cx);
            }
            return;
        }
        self.status = status.to_string();
        let (dot_color, show_stop) = match status {
            "listening" => (vec4(0.098, 0.725, 0.506, 1.0), true),
            "warming" => (vec4(0.906, 0.620, 0.204, 1.0), false),
            _ => (vec4(0.451, 0.463, 0.478, 1.0), false),
        };
        self.view
            .view(ids!(overlay_footer.footer_controls.overlay_status_dot))
            .apply_over(cx, live! { draw_bg: { dot_color: (dot_color) } });
        self.view
            .button(ids!(overlay_footer.footer_controls.overlay_stop_btn))
            .set_visible(cx, show_stop);
        if status == "idle" {
            self.show_idle_placeholder_if_empty(cx);
        } else if self.idle_placeholder_visible {
            self.clear(cx);
        }
        self.view.redraw(cx);
    }

    fn compute_anchor_spacer_height(
        viewport_height: f64,
        content_without_spacer: f64,
        anchor_ratio: f64,
    ) -> f32 {
        // User rule: follow starts only after content exceeds the chosen anchor position.
        if content_without_spacer <= viewport_height * anchor_ratio {
            return 0.0;
        }

        // To place the latest line at `anchor_ratio` from the top after scrolling
        // to the bottom, the bottom spacer must consume the remaining lower area.
        ((viewport_height * (1.0 - anchor_ratio)) - Self::TAIL_SAFE_GAP).max(0.0) as f32
    }

    fn update_anchor_spacer_from_layout(&mut self, cx: &mut Cx2d) {
        let measured_viewport_h = self
            .view
            .view(ids!(content_scroll))
            .area()
            .rect(cx)
            .size
            .y
            .max(0.0);
        let viewport_h = if self.viewport_height > 1.0 {
            self.viewport_height
        } else {
            measured_viewport_h
        };

        let history_height = self
            .view
            .translation_history(ids!(content_scroll.history_flow))
            .area()
            .rect(cx)
            .size
            .y
            .max(0.0);
        let pending_height = self
            .view
            .label(ids!(content_scroll.pending_label))
            .area()
            .rect(cx)
            .size
            .y
            .max(0.0);
        let pending_block_h = if self.pending_active {
            Self::PENDING_MARGIN_TOP + pending_height + Self::PENDING_MARGIN_BOTTOM
        } else {
            0.0
        };
        let content_without_spacer = Self::SCROLL_PADDING_TOP + history_height + pending_block_h;
        let anchor_ratio = Self::anchor_position_ratio(&self.anchor_position_preset);
        self.follow_tail_scroll = content_without_spacer > viewport_h * anchor_ratio;
        let mut spacer_h =
            Self::compute_anchor_spacer_height(viewport_h, content_without_spacer, anchor_ratio);

        // Hard guard for first ASR line: when only pending text exists,
        // keep top-flow and disable center-follow.
        if self.pending_only_mode {
            self.follow_tail_scroll = false;
            spacer_h = 0.0;
        }
        let changed = (self.last_spacer_height - spacer_h as f64).abs() >= 0.5;
        self.last_spacer_height = spacer_h as f64;
        // Always write spacer (including 0) to avoid stale initial/default values.
        self.view
            .view(ids!(content_scroll.bottom_spacer))
            .apply_over(
                cx,
                live! {
                    height: (spacer_h)
                },
            );
        if changed {
            // Spacer changed => request one more bottom snap on the next frame so
            // scroll position matches the new layout.
            self.pending_scroll = true;
            self.view.redraw(cx);
        }
    }

    /// Update the content viewport height (window height minus the footer).
    pub fn set_viewport_height(&mut self, cx: &mut Cx, viewport_height: f64) {
        self.viewport_height = viewport_height;
        self.last_spacer_height = -1.0; // force recompute on next draw
        self.view.redraw(cx);
    }

    pub fn set_font_size_preset(&mut self, cx: &mut Cx, preset: &str) {
        let normalized = if Self::FONT_SIZE_PRESETS.contains(&preset) {
            preset
        } else {
            "24"
        };
        if self.font_size_preset == normalized {
            return;
        }
        self.font_size_preset = normalized.to_string();
        self.update_font_size_draw_styles(cx);
        self.view.redraw(cx);
    }

    pub fn set_footer_font_size_preset(&mut self, cx: &mut Cx, preset: &str) {
        let normalized = if Self::FOOTER_FONT_SIZE_PRESETS.contains(&preset) {
            preset
        } else {
            "10"
        };
        if self.footer_font_size_preset == normalized {
            return;
        }
        self.footer_font_size_preset = normalized.to_string();
        self.update_footer_font_size_draw_styles(cx);
        self.view.redraw(cx);
    }

    pub fn set_anchor_position_preset(&mut self, cx: &mut Cx, preset: &str) {
        let normalized = match preset {
            "35" | "50" | "70" | "100" => preset,
            _ => "50",
        };
        if self.anchor_position_preset == normalized {
            return;
        }
        self.anchor_position_preset = normalized.to_string();
        self.last_spacer_height = -1.0;
        self.pending_scroll = true;
        self.view.redraw(cx);
    }

    /// Set passthrough mode (no translation — source text is the final output).
    pub fn set_passthrough(&mut self, _cx: &mut Cx, passthrough: bool) {
        self.passthrough = passthrough;
    }

    /// Switch between the split dual-language view and the classic interleaved list.
    pub fn set_split_view(&mut self, cx: &mut Cx, split: bool) {
        if self.split_mode == split {
            return;
        }
        self.split_mode = split;
        self.view.view(ids!(split_view)).set_visible(cx, split);
        self.view.view(ids!(content_scroll)).set_visible(cx, !split);
        // Force the newly shown view to rebuild from the next update.
        self.last_split_translation.clear();
        self.last_split_source.clear();
        self.last_history_len = 0;
        self.last_pending_text.clear();
        self.last_spacer_height = -1.0;
        self.trans_scroll_cur = 0.0;
        self.trans_scroll_target = 0.0;
        self.src_scroll_cur = 0.0;
        self.src_scroll_target = 0.0;
        self.pending_scroll = true;
        if !self.has_runtime_content() {
            self.idle_placeholder_visible = false;
            self.show_idle_placeholder_if_empty(cx);
        }
        self.view.redraw(cx);
    }

    pub fn set_language_pair(&mut self, cx: &mut Cx, source_lang: &str, target_lang: &str) {
        let passthrough = target_lang.eq_ignore_ascii_case("none");
        self.passthrough = passthrough;
        self.source_lang_code = source_lang.to_string();
        self.target_lang_code = target_lang.to_string();
        self.update_split_chips(cx);
        let placeholder_lang = if passthrough {
            source_lang
        } else {
            target_lang
        };
        if self.placeholder_lang != placeholder_lang {
            self.placeholder_lang = placeholder_lang.to_string();
        }
        if !self.has_runtime_content() || self.idle_placeholder_visible {
            self.show_idle_placeholder_if_empty(cx);
        }
    }

    /// Set overlay background opacity (0.0 = fully transparent, 1.0 = opaque).
    pub fn set_opacity(&mut self, cx: &mut Cx, opacity: f64) {
        self.view.apply_over(
            cx,
            live! {
                draw_bg: { bg_opacity: (opacity) }
            },
        );
        self.view.redraw(cx);
    }

    /// Render translation history and pending ASR text.
    ///
    /// `history` — completed sentences as `(source_text, translation)` pairs.
    /// `pending` — current in-progress ASR text (not yet translated).
    pub fn set_translation_update(
        &mut self,
        cx: &mut Cx,
        history: &[(String, String)],
        pending: &str,
    ) {
        self.idle_placeholder_visible = false;
        if self.split_mode {
            let (translation, source) =
                Self::build_split_texts(history, pending.trim(), self.passthrough);
            self.apply_split_update(cx, translation, source);
            return;
        }
        let history_segments = Self::format_history_segments(history, self.passthrough);
        self.view
            .translation_history(ids!(content_scroll.history_flow))
            .set_segments(cx, history_segments);

        // Pending ASR text
        let pending = pending.trim();
        let pending_label = self.view.label(ids!(content_scroll.pending_label));
        if pending.is_empty() {
            pending_label.set_text(cx, "");
            pending_label.apply_over(cx, live! { margin: { top: 0.0, bottom: 0.0 } });
            self.pending_active = false;
        } else {
            pending_label.set_text(cx, pending);
            pending_label.apply_over(cx, live! { margin: { top: 8.0, bottom: 8.0 } });
            self.pending_active = true;
        }
        self.pending_only_mode = history.is_empty() && self.pending_active;
        if self.pending_only_mode {
            // Hard reset on first pending line: never reuse previous scroll state.
            self.follow_tail_scroll = false;
            self.pending_scroll = true;
        }

        let new_len = history.len();
        let pending_changed = pending != self.last_pending_text;
        if self.pending_only_mode || new_len != self.last_history_len || pending_changed {
            self.last_history_len = new_len;
            self.last_pending_text = pending.to_string();
            // Mark scroll pending — actual set_scroll_pos happens in draw_walk
            // AFTER view.draw_walk() so that view_total reflects new content.
            self.pending_scroll = true;
        }

        self.view.redraw(cx);
    }

    fn format_history_segments(
        history: &[(String, String)],
        passthrough: bool,
    ) -> Vec<HistorySegment> {
        let mut segments = Vec::new();
        for (source, translation) in history {
            if passthrough {
                // No translation — source text is the output, shown in white.
                segments.push(HistorySegment::translation(source.trim()));
            } else {
                let translation_trimmed = translation.trim();
                if translation_trimmed.is_empty() {
                    // Translation not yet available — show source as fallback.
                    segments.push(HistorySegment::source(source.trim()));
                } else {
                    segments.push(HistorySegment::source(source.trim()));
                    segments.push(HistorySegment::translation(translation_trimmed));
                }
            }
        }
        segments
    }

    /// Clear all text and reset state.
    pub fn clear(&mut self, cx: &mut Cx) {
        self.last_history_len = 0;
        self.last_pending_text.clear();
        self.idle_placeholder_visible = false;
        self.pending_active = false;
        self.follow_tail_scroll = false;
        self.pending_only_mode = false;
        self.pending_scroll = false;
        self.last_spacer_height = 0.0;
        self.last_split_translation.clear();
        self.last_split_source.clear();
        self.trans_scroll_cur = 0.0;
        self.trans_scroll_target = 0.0;
        self.src_scroll_cur = 0.0;
        self.src_scroll_target = 0.0;
        self.view
            .label(ids!(split_view.translation_pane.translation_scroll.translation_text))
            .set_text(cx, "");
        self.view
            .label(ids!(split_view.source_pane.source_scroll.source_text_label))
            .set_text(cx, "");
        self.view
            .view(ids!(split_view.translation_pane.translation_scroll))
            .set_scroll_pos(cx, dvec2(0.0, 0.0));
        self.view
            .view(ids!(split_view.source_pane.source_scroll))
            .set_scroll_pos(cx, dvec2(0.0, 0.0));
        self.view
            .translation_history(ids!(content_scroll.history_flow))
            .set_segments(cx, Vec::new());
        let pending_label = self.view.label(ids!(content_scroll.pending_label));
        pending_label.set_text(cx, "");
        pending_label.apply_over(cx, live! { margin: { top: 0.0, bottom: 0.0 } });
        self.view.redraw(cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{HistorySegment, TranslationOverlay};

    #[test]
    fn anchor_spacer_shrinks_when_pending_height_grows() {
        let viewport = 216.0;
        let short_content = 100.0;
        let long_content = 280.0;
        let short_spacer =
            TranslationOverlay::compute_anchor_spacer_height(viewport, short_content, 0.5);
        let long_spacer =
            TranslationOverlay::compute_anchor_spacer_height(viewport, long_content, 0.5);
        assert!(short_spacer < long_spacer);
    }

    #[test]
    fn anchor_spacer_is_never_negative() {
        let spacer = TranslationOverlay::compute_anchor_spacer_height(216.0, 300.0, 0.5);
        assert!(spacer >= 0.0);
    }

    #[test]
    fn anchor_spacer_stays_zero_before_half_viewport() {
        let spacer = TranslationOverlay::compute_anchor_spacer_height(216.0, 108.0, 0.5);
        assert_eq!(spacer, 0.0);
    }

    #[test]
    fn font_size_preset_values_match_expected_scale() {
        assert_eq!(
            TranslationOverlay::font_size_preset_values("24"),
            (24.0, 23.0)
        );
        assert_eq!(
            TranslationOverlay::font_size_preset_values("36"),
            (36.0, 35.0)
        );
    }

    #[test]
    fn footer_font_size_value_parses_known_presets() {
        assert_eq!(TranslationOverlay::footer_font_size_value("10"), 10.0);
        assert_eq!(TranslationOverlay::footer_font_size_value("20"), 20.0);
        assert_eq!(TranslationOverlay::footer_font_size_value("16"), 16.0);
    }

    #[test]
    fn footer_logo_size_tracks_large_tagline_sizes() {
        assert_eq!(TranslationOverlay::footer_logo_size_value("10"), 22.0);
        assert_eq!(TranslationOverlay::footer_logo_size_value("22"), 22.0);
        assert_eq!(TranslationOverlay::footer_logo_size_value("30"), 30.0);
        assert_eq!(TranslationOverlay::footer_logo_size_value("32"), 32.0);
    }

    #[test]
    fn footer_font_size_presets_include_large_tagline_sizes() {
        for preset in ["22", "24", "26", "28", "30", "32"] {
            assert!(
                TranslationOverlay::FOOTER_FONT_SIZE_PRESETS.contains(&preset),
                "missing footer font size preset {preset}"
            );
        }
    }

    #[test]
    fn footer_font_size_value_falls_back_when_invalid() {
        assert_eq!(TranslationOverlay::footer_font_size_value(""), 20.0);
        assert_eq!(TranslationOverlay::footer_font_size_value("abc"), 20.0);
    }

    #[test]
    fn anchor_position_preset_values_match_expected_ratios() {
        assert_eq!(TranslationOverlay::anchor_position_ratio("35"), 0.35);
        assert_eq!(TranslationOverlay::anchor_position_ratio("50"), 0.5);
        assert_eq!(TranslationOverlay::anchor_position_ratio("70"), 0.7);
        assert_eq!(TranslationOverlay::anchor_position_ratio("100"), 1.0);
    }

    #[test]
    fn upper_anchor_creates_more_bottom_spacer_than_lower_anchor() {
        let upper = TranslationOverlay::compute_anchor_spacer_height(216.0, 300.0, 0.5);
        let lower = TranslationOverlay::compute_anchor_spacer_height(216.0, 300.0, 0.9);
        assert!(upper > lower);
    }

    #[test]
    fn format_history_passthrough_shows_source() {
        let items = vec![("hello".to_string(), "".to_string())];
        let segments = TranslationOverlay::format_history_segments(&items, true);
        assert_eq!(segments, vec![HistorySegment::translation("hello")]);

        let items = vec![
            ("hi".to_string(), "".to_string()),
            ("world".to_string(), "".to_string()),
        ];
        let segments = TranslationOverlay::format_history_segments(&items, true);
        assert_eq!(
            segments,
            vec![
                HistorySegment::translation("hi"),
                HistorySegment::translation("world"),
            ]
        );
    }

    #[test]
    fn format_history_translation_mode_shows_source_and_translation() {
        let items = vec![("hello".to_string(), "你好".to_string())];
        let segments = TranslationOverlay::format_history_segments(&items, false);
        assert_eq!(
            segments,
            vec![
                HistorySegment::source("hello"),
                HistorySegment::translation("你好")
            ]
        );

        let items = vec![
            ("hi".to_string(), "你好".to_string()),
            ("bye".to_string(), "再见".to_string()),
        ];
        let segments = TranslationOverlay::format_history_segments(&items, false);
        assert_eq!(
            segments,
            vec![
                HistorySegment::source("hi"),
                HistorySegment::translation("你好"),
                HistorySegment::source("bye"),
                HistorySegment::translation("再见"),
            ]
        );
    }

    #[test]
    fn format_history_fallback_to_source_when_translation_empty() {
        let items = vec![("hello".to_string(), "".to_string())];
        let segments = TranslationOverlay::format_history_segments(&items, false);
        assert_eq!(segments, vec![HistorySegment::source("hello")]);
    }
}

impl TranslationOverlayRef {
    /// Update with sentence history and pending ASR text
    pub fn set_translation_update(&self, cx: &mut Cx, history: &[(String, String)], pending: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_translation_update(cx, history, pending);
        }
    }

    /// Clear and reset to empty state
    pub fn clear(&self, cx: &mut Cx) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.clear(cx);
        }
    }

    pub fn set_font_size_preset(&self, cx: &mut Cx, preset: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_font_size_preset(cx, preset);
        }
    }

    pub fn set_footer_font_size_preset(&self, cx: &mut Cx, preset: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_footer_font_size_preset(cx, preset);
        }
    }

    pub fn set_anchor_position_preset(&self, cx: &mut Cx, preset: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_anchor_position_preset(cx, preset);
        }
    }

    pub fn set_passthrough(&self, cx: &mut Cx, passthrough: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_passthrough(cx, passthrough);
        }
    }

    pub fn set_split_view(&self, cx: &mut Cx, split: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_split_view(cx, split);
        }
    }

    pub fn set_opacity(&self, cx: &mut Cx, opacity: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_opacity(cx, opacity);
        }
    }

    pub fn set_status(&self, cx: &mut Cx, status: &str) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_status(cx, status);
        }
    }

    pub fn set_locale(&self, cx: &mut Cx, locale_en: bool) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_locale(cx, locale_en);
        }
    }

    pub fn set_viewport_height(&self, cx: &mut Cx, viewport_height: f64) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.set_viewport_height(cx, viewport_height);
        }
    }
}
