use makepad_widgets::drop_down::PopupMenuPosition;
use makepad_widgets::popup_menu::{
    PopupMenuAction, PopupMenuItem, PopupMenuItemAction, PopupMenuItemId,
};
use makepad_widgets::*;
use std::cell::RefCell;
use std::rc::Rc;

live_design! {
    link widgets;
    use link::theme::*;
    use link::widgets::*;
    use makepad_draw::shader::std::*;

    pub BoundedDrawLabelText = {{BoundedDrawLabelText}} {}

    pub BoundedPopupMenuBase = {{BoundedPopupMenu}} {
        width: 150, height: Fit
        flow: Down
        padding: 0
        max_height: 320
        item_height: 32
        menu_item: <PopupMenuItem> {}
        draw_bg: {
            instance dark_mode: 0.0
            uniform border_size: 0.0
            uniform border_radius: 8.0
            uniform color_light: vec4(1.0, 1.0, 1.0, 1.0)
            uniform color_dark: vec4(0.118, 0.145, 0.196, 1.0)
            uniform border_light: vec4(0.796, 0.835, 0.878, 1.0)
            uniform border_dark: vec4(0.278, 0.337, 0.412, 1.0)
            uniform shadow_light: vec4(0.04, 0.08, 0.16, 0.42)
            uniform shadow_dark: vec4(0.0, 0.0, 0.0, 0.68)
            uniform shadow_radius: 16.0
            uniform shadow_offset: vec2(0.0, 5.0)

            varying rect_size2: vec2
            varying rect_size3: vec2
            varying rect_pos2: vec2
            varying rect_shift: vec2
            varying sdf_rect_pos: vec2
            varying sdf_rect_size: vec2

            fn vertex(self) -> vec4 {
                let min_offset = min(self.shadow_offset, vec2(0.0));
                self.rect_size2 = self.rect_size + 2.0 * vec2(self.shadow_radius);
                self.rect_size3 = self.rect_size2 + abs(self.shadow_offset);
                self.rect_pos2 = self.rect_pos - vec2(self.shadow_radius) + min_offset;
                self.sdf_rect_size = self.rect_size2 - vec2(self.shadow_radius * 2.0 + self.border_size * 2.0);
                self.sdf_rect_pos = -min_offset + vec2(self.border_size + self.shadow_radius);
                self.rect_shift = -min_offset;
                return self.clip_and_transform_vertex(self.rect_pos2, self.rect_size3);
            }

            fn pixel(self) -> vec4 {
                let sdf = Sdf2d::viewport(self.pos * self.rect_size3);
                sdf.box(
                    self.sdf_rect_pos.x,
                    self.sdf_rect_pos.y,
                    self.sdf_rect_size.x,
                    self.sdf_rect_size.y,
                    max(1.0, self.border_radius)
                );
                if sdf.shape > -1.0 {
                    let radius = self.shadow_radius;
                    let offset = self.shadow_offset + self.rect_shift;
                    let shadow = GaussShadow::rounded_box_shadow(
                        vec2(radius) + offset,
                        self.rect_size2 + offset,
                        self.pos * (self.rect_size3 + vec2(radius)),
                        radius * 0.5,
                        self.border_radius * 2.0
                    );
                    sdf.clear(mix(self.shadow_light, self.shadow_dark, self.dark_mode) * shadow);
                }
                sdf.fill_keep(mix(self.color_light, self.color_dark, self.dark_mode));
                if self.border_size > 0.0 {
                    sdf.stroke(mix(self.border_light, self.border_dark, self.dark_mode), self.border_size);
                }
                return sdf.result;
            }
        }
        scroll_bars: <ScrollBars> {
            show_scroll_x: false
            show_scroll_y: true
            scroll_bar_y: {
                bar_size: 8
                bar_side_margin: 2
                min_handle_size: 36
                draw_bg: {
                    size: 5
                    color: vec4(0.55, 0.60, 0.68, 0.72)
                    color_hover: vec4(0.23, 0.44, 0.83, 0.92)
                    color_drag: vec4(0.18, 0.36, 0.72, 1.0)
                }
            }
        }
    }

    pub BoundedDropDownBase = {{BoundedDropDown}} {
        width: Fit, height: Fit
        align: {x: 0, y: 0}
        padding: {left: 6, right: 22.5, top: 3, bottom: 3}
        margin: {left: 0, right: 0, top: 3, bottom: 3}
        popup_menu_position: BelowInput
        popup_menu: <BoundedPopupMenuBase> {}
        selected_item: 0

        draw_bg: {
            instance hover: 0.0
            instance focus: 0.0
            instance down: 0.0
            instance active: 0.0
            instance disabled: 0.0
            uniform border_size: 0.0
            uniform border_radius: 0.0
            fn pixel(self) -> vec4 {
                return vec4(0.0, 0.0, 0.0, 0.0);
            }
        }
        draw_text: <BoundedDrawLabelText> {
            fn get_color(self) -> vec4 {
                return vec4(0.0, 0.0, 0.0, 1.0);
            }
        }

        animator: {
            disabled = {
                default: off
                off = {
                    from: {all: Forward {duration: 0}}
                    apply: {draw_bg: {disabled: 0} draw_text: {disabled: 0}}
                }
                on = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {draw_bg: {disabled: 1} draw_text: {disabled: 1}}
                }
            }
            hover = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.1}}
                    apply: {draw_bg: {down: 0, hover: 0} draw_text: {down: 0, hover: 0}}
                }
                on = {
                    from: {all: Forward {duration: 0.1} down: Forward {duration: 0.01}}
                    apply: {draw_bg: {down: 0, hover: 1} draw_text: {down: 0, hover: 1}}
                }
                down = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {draw_bg: {down: 1, hover: 1} draw_text: {down: 1, hover: 1}}
                }
            }
            focus = {
                default: off
                off = {
                    from: {all: Forward {duration: 0.2}}
                    apply: {draw_bg: {focus: 0} draw_text: {focus: 0}}
                }
                on = {
                    cursor: Arrow
                    from: {all: Forward {duration: 0}}
                    apply: {draw_bg: {focus: 1} draw_text: {focus: 1}}
                }
            }
        }
    }
}

#[derive(Live, LiveHook, LiveRegister)]
#[repr(C)]
pub struct BoundedDrawLabelText {
    #[deref]
    draw_super: DrawText,
    #[live]
    focus: f32,
    #[live]
    hover: f32,
    #[live]
    down: f32,
    #[live]
    disabled: f32,
}

#[derive(Live, LiveRegister)]
pub struct BoundedPopupMenu {
    #[live]
    draw_list: DrawList2d,
    #[live]
    menu_item: Option<LivePtr>,
    #[live]
    draw_bg: DrawQuad,
    #[layout]
    layout: Layout,
    #[walk]
    walk: Walk,
    #[live]
    scroll_bars: ScrollBars,
    #[live(320.0)]
    max_height: f64,
    #[live(32.0)]
    item_height: f64,
    #[rust]
    first_tap: bool,
    #[rust]
    menu_items: ComponentMap<PopupMenuItemId, PopupMenuItem>,
    #[rust]
    init_select_item: Option<PopupMenuItemId>,
    #[rust]
    count: usize,
}

impl LiveHook for BoundedPopupMenu {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, index: usize, nodes: &[LiveNode]) {
        if let Some(index) = nodes.child_by_name(index, live_id!(list_node).as_field()) {
            for (_, node) in self.menu_items.iter_mut() {
                node.apply(cx, apply, index, nodes);
            }
        }
        self.draw_list.redraw(cx);
    }
}

impl BoundedPopupMenu {
    fn desired_height(&self, item_count: usize) -> f64 {
        (item_count as f64 * self.item_height + self.layout.padding.height()).min(self.max_height)
    }

    fn begin(&mut self, cx: &mut Cx2d, width: f64, height: f64) {
        self.draw_list.begin_overlay_reuse(cx);
        cx.begin_root_turtle(cx.current_pass_size(), Layout::flow_down());
        self.draw_bg.begin(
            cx,
            Walk::fixed(width.max(1.0), height.max(self.item_height)),
            self.layout,
        );
        self.scroll_bars
            .begin(cx, Walk::fill(), Layout::flow_down());
        self.count = 0;
    }

    fn end(&mut self, cx: &mut Cx2d, shift_area: Area, shift: Vec2d) {
        self.scroll_bars.end(cx);
        self.draw_bg.end(cx);
        cx.end_pass_sized_turtle_with_shift(shift_area, shift);
        self.draw_list.end(cx);
        self.menu_items.retain_visible();
        if let Some(init_select_item) = self.init_select_item.take() {
            self.select_item_state(cx, init_select_item);
        }
    }

    fn draw_item(&mut self, cx: &mut Cx2d, item_id: PopupMenuItemId, label: &str) {
        self.count += 1;
        let menu_item = self.menu_item;
        self.menu_items
            .get_or_insert(cx, item_id, |cx| PopupMenuItem::new_from_ptr(cx, menu_item))
            .draw_item(cx, label);
    }

    fn init_select_item(&mut self, which_id: PopupMenuItemId) {
        self.init_select_item = Some(which_id);
        self.first_tap = true;
    }

    fn select_item_state(&mut self, cx: &mut Cx, which_id: PopupMenuItemId) {
        for (id, item) in &mut *self.menu_items {
            if *id == which_id {
                item.animator_cut(cx, ids!(active.on));
                item.animator_cut(cx, ids!(hover.on));
            } else {
                item.animator_cut(cx, ids!(active.off));
                item.animator_cut(cx, ids!(hover.off));
            }
        }
    }

    fn menu_contains_pos(&self, cx: &mut Cx, pos: Vec2d) -> bool {
        self.draw_bg.area().clipped_rect(cx).contains(pos)
    }

    fn handle_event_with(
        &mut self,
        cx: &mut Cx,
        event: &Event,
        scope: &mut Scope,
        sweep_area: Area,
        dispatch_action: &mut dyn FnMut(&mut Cx, PopupMenuAction),
    ) {
        self.scroll_bars.handle_event(cx, event, scope);

        let mut actions = Vec::new();
        for (item_id, node) in self.menu_items.iter_mut() {
            node.handle_event_with(cx, event, sweep_area, &mut |_, action| {
                actions.push((*item_id, action))
            });
        }

        for (node_id, action) in actions {
            match action {
                PopupMenuItemAction::MightBeSelected => {
                    if self.first_tap {
                        self.first_tap = false;
                    } else {
                        self.select_item_state(cx, node_id);
                        dispatch_action(cx, PopupMenuAction::WasSelected(node_id));
                    }
                }
                PopupMenuItemAction::WasSweeped => {
                    self.select_item_state(cx, node_id);
                    dispatch_action(cx, PopupMenuAction::WasSweeped(node_id));
                }
                PopupMenuItemAction::WasSelected => {
                    self.select_item_state(cx, node_id);
                    dispatch_action(cx, PopupMenuAction::WasSelected(node_id));
                }
                PopupMenuItemAction::None => {}
            }
        }
    }
}

#[derive(Live, Widget)]
pub struct BoundedDropDown {
    #[animator]
    animator: Animator,
    #[redraw]
    #[live]
    draw_bg: DrawQuad,
    #[live]
    draw_text: BoundedDrawLabelText,
    #[walk]
    walk: Walk,
    #[live]
    bind: String,
    #[live]
    bind_enum: String,
    #[live]
    popup_menu: Option<LivePtr>,
    #[live]
    labels: Vec<String>,
    #[live]
    values: Vec<LiveValue>,
    #[live]
    popup_menu_position: PopupMenuPosition,
    #[rust]
    is_active: bool,
    #[rust]
    is_disabled: bool,
    #[live]
    selected_item: usize,
    #[layout]
    layout: Layout,
}

#[derive(Default, Clone)]
struct BoundedPopupMenuGlobal {
    map: Rc<RefCell<ComponentMap<LivePtr, BoundedPopupMenu>>>,
}

impl LiveHook for BoundedDropDown {
    fn after_apply(&mut self, cx: &mut Cx, apply: &mut Apply, _index: usize, _nodes: &[LiveNode]) {
        if self.popup_menu.is_none() || !apply.from.is_from_doc() {
            return;
        }
        let global = cx.global::<BoundedPopupMenuGlobal>().clone();
        let mut map = global.map.borrow_mut();
        map.retain(|ptr, _| cx.live_registry.borrow().generation_valid(*ptr));
        let popup = self.popup_menu.unwrap();
        map.get_or_insert(cx, popup, |cx| {
            BoundedPopupMenu::new_from_ptr(cx, Some(popup))
        });
    }
}

#[derive(Clone, Debug, DefaultNone)]
pub enum BoundedDropDownAction {
    Select(usize, LiveValue),
    None,
}

impl BoundedDropDown {
    const POPUP_EDGE_GAP: f64 = 16.0;
    const POPUP_CONTENT_CHROME: f64 = 48.0;

    fn set_active(&mut self, cx: &mut Cx) {
        self.is_active = true;
        self.draw_bg.apply_over(cx, live! {active: 1.0});
        self.draw_bg.redraw(cx);
        let global = cx.global::<BoundedPopupMenuGlobal>().clone();
        let mut map = global.map.borrow_mut();
        let menu = map.get_mut(&self.popup_menu.unwrap()).unwrap();
        menu.init_select_item(LiveId(self.selected_item as u64).into());
        menu.scroll_bars.set_scroll_pos(cx, dvec2(0.0, 0.0));
        cx.sweep_lock(self.draw_bg.area());
    }

    fn set_closed(&mut self, cx: &mut Cx) {
        self.is_active = false;
        self.draw_bg.apply_over(cx, live! {active: 0.0});
        self.draw_bg.redraw(cx);
        cx.sweep_unlock(self.draw_bg.area());
    }

    fn popup_geometry(&self, cx: &mut Cx2d, desired_height: f64) -> (f64, f64) {
        let trigger = self.draw_bg.area().rect(cx);
        let pass_height = cx.current_pass_size().y;
        let edge_gap = Self::POPUP_EDGE_GAP;
        let available_below = (pass_height - trigger.pos.y - trigger.size.y - edge_gap).max(0.0);
        let available_above = (trigger.pos.y - edge_gap).max(0.0);
        let minimum_useful_height = 4.0 * 32.0;

        if available_below >= desired_height.min(minimum_useful_height)
            || available_below >= available_above
        {
            (desired_height.min(available_below), trigger.size.y)
        } else {
            let height = desired_height.min(available_above);
            (height, -height)
        }
    }

    fn popup_width_geometry(&self, cx: &mut Cx2d, content_width: f64) -> (f64, f64) {
        let trigger = self.draw_bg.area().rect(cx);
        let pass_width = cx.current_pass_size().x;
        let edge_gap = Self::POPUP_EDGE_GAP;
        let maximum_width = (pass_width - edge_gap * 2.0).max(96.0);
        let width = content_width.clamp(96.0, maximum_width);

        if trigger.pos.x + width + edge_gap <= pass_width {
            (width, 0.0)
        } else if trigger.pos.x + trigger.size.x - width >= edge_gap {
            (width, trigger.size.x - width)
        } else {
            (width, edge_gap - trigger.pos.x)
        }
    }

    fn draw_bounded(&mut self, cx: &mut Cx2d, walk: Walk) {
        self.draw_bg.begin(cx, walk, self.layout);
        let label = self
            .labels
            .get(self.selected_item)
            .map(String::as_str)
            .unwrap_or(" ");
        self.draw_text
            .draw_walk(cx, Walk::fit(), Align::default(), label);
        self.draw_bg.end(cx);
        cx.add_nav_stop(self.draw_bg.area(), NavRole::DropDown, Margin::default());

        if !self.is_active || self.popup_menu.is_none() {
            return;
        }

        let longest_label_width = self
            .labels
            .iter()
            .map(|label| {
                self.draw_text
                    .layout(cx, 0.0, 0.0, None, false, Align::default(), label)
                    .size_in_lpxs
                    .width as f64
                    * self.draw_text.font_scale as f64
            })
            .fold(0.0, f64::max);
        let (menu_width, shift_x) =
            self.popup_width_geometry(cx, longest_label_width + Self::POPUP_CONTENT_CHROME);
        let global = cx.global::<BoundedPopupMenuGlobal>().clone();
        let mut map = global.map.borrow_mut();
        let menu = map.get_mut(&self.popup_menu.unwrap()).unwrap();
        let desired_height = menu.desired_height(self.labels.len());
        let (menu_height, shift_y) = self.popup_geometry(cx, desired_height);

        menu.begin(cx, menu_width, menu_height);
        for (index, item) in self.labels.iter().enumerate() {
            menu.draw_item(cx, LiveId(index as u64).into(), item);
        }
        menu.end(cx, self.draw_bg.area(), dvec2(shift_x, shift_y));
    }
}

impl Widget for BoundedDropDown {
    fn set_disabled(&mut self, cx: &mut Cx, disabled: bool) {
        self.is_disabled = disabled;
        self.animator_toggle(
            cx,
            disabled,
            Animate::Yes,
            ids!(disabled.on),
            ids!(disabled.off),
        );
    }

    fn disabled(&self, cx: &Cx) -> bool {
        self.animator_in_state(cx, ids!(disabled.on))
    }

    fn widget_to_data(
        &self,
        _cx: &mut Cx,
        actions: &Actions,
        nodes: &mut LiveNodeVec,
        path: &[LiveId],
    ) -> bool {
        if let BoundedDropDownAction::Select(_, value) =
            actions.find_widget_action_cast(self.widget_uid())
        {
            nodes.write_field_value(path, value);
            true
        } else {
            false
        }
    }

    fn data_to_widget(&mut self, cx: &mut Cx, nodes: &[LiveNode], path: &[LiveId]) {
        if let Some(value) = nodes.read_field_value(path) {
            if let Some(index) = self.values.iter().position(|candidate| candidate == value) {
                if self.selected_item != index {
                    self.selected_item = index;
                    self.draw_bg.redraw(cx);
                }
            }
        }
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        self.animator_handle_event(cx, event);
        let uid = self.widget_uid();

        if self.is_active && self.popup_menu.is_some() {
            let action_path = scope.path.clone();
            let global = cx.global::<BoundedPopupMenuGlobal>().clone();
            let mut map = global.map.borrow_mut();
            let menu = map.get_mut(&self.popup_menu.unwrap()).unwrap();
            let mut close = false;
            menu.handle_event_with(cx, event, scope, self.draw_bg.area(), &mut |cx, action| {
                if let PopupMenuAction::WasSelected(node_id) = action {
                    self.selected_item = node_id.0 .0 as usize;
                    let value = self
                        .values
                        .get(self.selected_item)
                        .cloned()
                        .unwrap_or(LiveValue::None);
                    cx.widget_action(
                        uid,
                        &action_path,
                        BoundedDropDownAction::Select(self.selected_item, value),
                    );
                    self.draw_bg.redraw(cx);
                    close = true;
                }
            });
            if close {
                self.set_closed(cx);
            }
            if let Event::MouseDown(event) = event {
                if !menu.menu_contains_pos(cx, event.abs) {
                    self.set_closed(cx);
                    self.animator_play(cx, ids!(hover.off));
                    return;
                }
            }
        }

        match event.hits_with_sweep_area(cx, self.draw_bg.area(), self.draw_bg.area()) {
            Hit::KeyFocusLost(_) => {
                self.animator_play(cx, ids!(focus.off));
                self.set_closed(cx);
                self.animator_play(cx, ids!(hover.off));
            }
            Hit::KeyFocus(_) => self.animator_play(cx, ids!(focus.on)),
            Hit::KeyDown(event) => match event.key_code {
                KeyCode::ArrowUp if self.selected_item > 0 => {
                    self.selected_item -= 1;
                    let value = self
                        .values
                        .get(self.selected_item)
                        .cloned()
                        .unwrap_or(LiveValue::None);
                    cx.widget_action(
                        uid,
                        &scope.path,
                        BoundedDropDownAction::Select(self.selected_item, value),
                    );
                    self.set_closed(cx);
                    self.draw_bg.redraw(cx);
                }
                KeyCode::ArrowDown if self.selected_item + 1 < self.values.len() => {
                    self.selected_item += 1;
                    let value = self
                        .values
                        .get(self.selected_item)
                        .cloned()
                        .unwrap_or(LiveValue::None);
                    cx.widget_action(
                        uid,
                        &scope.path,
                        BoundedDropDownAction::Select(self.selected_item, value),
                    );
                    self.set_closed(cx);
                    self.draw_bg.redraw(cx);
                }
                _ => {}
            },
            Hit::FingerDown(event) if event.is_primary_hit() => {
                if !self.is_disabled {
                    cx.set_key_focus(self.draw_bg.area());
                    self.animator_play(cx, ids!(hover.down));
                    self.set_active(cx);
                }
            }
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
                self.animator_play(cx, ids!(hover.on));
            }
            Hit::FingerHoverOut(_) => self.animator_play(cx, ids!(hover.off)),
            Hit::FingerUp(event) if event.is_primary_hit() => {
                self.animator_play(
                    cx,
                    if event.is_over {
                        ids!(hover.on)
                    } else {
                        ids!(hover.off)
                    },
                );
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        self.draw_bounded(cx, walk);
        DrawStep::done()
    }
}

impl BoundedDropDownRef {
    pub fn set_labels(&self, cx: &mut Cx, labels: Vec<String>) {
        if let Some(mut inner) = self.borrow_mut() {
            inner.labels = labels;
            inner.draw_bg.redraw(cx);
        }
    }

    pub fn changed(&self, actions: &Actions) -> Option<usize> {
        actions
            .find_widget_action(self.widget_uid())
            .and_then(|action| match action.cast() {
                BoundedDropDownAction::Select(index, _) => Some(index),
                BoundedDropDownAction::None => None,
            })
    }

    pub fn set_selected_item(&self, cx: &mut Cx, item: usize) {
        if let Some(mut inner) = self.borrow_mut() {
            let selected = item.min(inner.labels.len().max(1) - 1);
            if selected != inner.selected_item {
                inner.selected_item = selected;
                inner.draw_bg.redraw(cx);
            }
        }
    }

    pub fn selected_item(&self) -> usize {
        self.borrow()
            .map(|inner| inner.selected_item)
            .unwrap_or_default()
    }

    pub fn selected_label(&self) -> String {
        self.borrow()
            .and_then(|inner| inner.labels.get(inner.selected_item).cloned())
            .unwrap_or_default()
    }
}
