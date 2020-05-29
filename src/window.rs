use crate::{
    message::{
        UiMessageData,
        UiMessage,
    },
    border::BorderBuilder,
    UINode,
    UserInterface,
    grid::{
        GridBuilder,
        Column,
        Row,
    },
    HorizontalAlignment,
    text::TextBuilder,
    Thickness,
    button::ButtonBuilder,
    widget::{
        Widget,
        WidgetBuilder,
    },
    Control,
    core::{
        pool::Handle,
        math::{
            vec2::Vec2,
            Rect,
        },
        color::Color,
    },
    message::{
        WidgetMessage,
        ButtonMessage,
        WindowMessage,
    },
    brush::{
        Brush,
        GradientPoint,
    },
    NodeHandleMapping,
};
use std::ops::{Deref, DerefMut};
use std::cell::RefCell;

/// Represents a widget looking as window in Windows - with title, minimize and close buttons.
/// It has scrollable region for content, content can be any desired node or even other window.
/// Window can be dragged by its title.
pub struct Window<M: 'static, C: 'static + Control<M, C>> {
    widget: Widget<M, C>,
    mouse_click_pos: Vec2,
    initial_position: Vec2,
    initial_size: Vec2,
    is_dragging: bool,
    minimized: bool,
    can_minimize: bool,
    can_close: bool,
    header: Handle<UINode<M, C>>,
    minimize_button: Handle<UINode<M, C>>,
    close_button: Handle<UINode<M, C>>,
    drag_delta: Vec2,
    content: Handle<UINode<M, C>>,
    grips: RefCell<[Grip; 8]>,
}

const GRIP_SIZE: f32 = 6.0;
const CORNER_GRIP_SIZE: f32 = GRIP_SIZE * 2.0;

#[derive(Copy, Clone, Debug)]
enum GripKind {
    LeftTopCorner = 0,
    RightTopCorner = 1,
    RightBottomCorner = 2,
    LeftBottomCorner = 3,
    Left = 4,
    Top = 5,
    Right = 6,
    Bottom = 7,
}

#[derive(Clone)]
struct Grip {
    kind: GripKind,
    bounds: Rect<f32>,
    is_dragging: bool,
}

impl Grip {
    fn new(kind: GripKind) -> Self {
        Self {
            kind,
            bounds: Default::default(),
            is_dragging: false,
        }
    }
}

impl<M: 'static, C: 'static + Control<M, C>> Deref for Window<M, C> {
    type Target = Widget<M, C>;

    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}

impl<M: 'static, C: 'static + Control<M, C>> DerefMut for Window<M, C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.widget
    }
}

impl<M: 'static, C: 'static + Control<M, C>> Clone for Window<M, C> {
    fn clone(&self) -> Self {
        Self {
            widget: self.widget.raw_copy(),
            mouse_click_pos: self.mouse_click_pos,
            initial_position: self.initial_position,
            is_dragging: self.is_dragging,
            minimized: self.minimized,
            can_minimize: self.can_minimize,
            can_close: self.can_close,
            header: self.header,
            minimize_button: self.minimize_button,
            close_button: self.close_button,
            drag_delta: self.drag_delta,
            content: self.content,
            grips: self.grips.clone(),
            initial_size: self.initial_size,
        }
    }
}

impl<M, C: 'static + Control<M, C>> Control<M, C> for Window<M, C> {
    fn raw_copy(&self) -> UINode<M, C> {
        UINode::Window(self.clone())
    }

    fn resolve(&mut self, node_map: &NodeHandleMapping<M, C>) {
        self.header = *node_map.get(&self.header).unwrap();
        self.minimize_button = *node_map.get(&self.minimize_button).unwrap();
        self.close_button = *node_map.get(&self.close_button).unwrap();
        if self.content.is_some() {
            self.content = *node_map.get(&self.content).unwrap();
        }
    }

    fn arrange_override(&self, ui: &UserInterface<M, C>, final_size: Vec2) -> Vec2 {
        let size = self.widget.arrange_override(ui, final_size);

        let mut grips = self.grips.borrow_mut();

        // Adjust grips.
        grips[GripKind::Left as usize].bounds = Rect {
            x: 0.0,
            y: GRIP_SIZE,
            w: GRIP_SIZE,
            h: final_size.y - GRIP_SIZE * 2.0,
        };
        grips[GripKind::Top as usize].bounds = Rect {
            x: GRIP_SIZE,
            y: 0.0,
            w: final_size.x - GRIP_SIZE * 2.0,
            h: GRIP_SIZE,
        };
        grips[GripKind::Right as usize].bounds = Rect {
            x: final_size.x - GRIP_SIZE,
            y: GRIP_SIZE,
            w: GRIP_SIZE,
            h: final_size.y - GRIP_SIZE * 2.0,
        };
        grips[GripKind::Bottom as usize].bounds = Rect {
            x: GRIP_SIZE,
            y: final_size.y - GRIP_SIZE,
            w: final_size.x - GRIP_SIZE * 2.0,
            h: GRIP_SIZE,
        };

        // Corners have different size to improve usability.
        grips[GripKind::LeftTopCorner as usize].bounds = Rect {
            x: 0.0,
            y: 0.0,
            w: CORNER_GRIP_SIZE,
            h: CORNER_GRIP_SIZE,
        };
        grips[GripKind::RightTopCorner as usize].bounds = Rect {
            x: final_size.x - GRIP_SIZE,
            y: 0.0,
            w: CORNER_GRIP_SIZE,
            h: CORNER_GRIP_SIZE,
        };
        grips[GripKind::RightBottomCorner as usize].bounds = Rect {
            x: final_size.x - CORNER_GRIP_SIZE,
            y: final_size.y - CORNER_GRIP_SIZE,
            w: CORNER_GRIP_SIZE,
            h: CORNER_GRIP_SIZE,
        };
        grips[GripKind::LeftBottomCorner as usize].bounds = Rect {
            x: 0.0,
            y: final_size.y - CORNER_GRIP_SIZE,
            w: CORNER_GRIP_SIZE,
            h: CORNER_GRIP_SIZE,
        };

        size
    }

    fn handle_routed_message(&mut self, ui: &mut UserInterface<M, C>, message: &mut UiMessage<M, C>) {
        self.widget.handle_routed_message(ui, message);

        match &message.data {
            UiMessageData::Widget(msg) => {
                // Grip interaction have higher priority than other actions.
                match msg {
                    &WidgetMessage::MouseDown { pos, .. } => {
                        self.send_message(UiMessage {
                            data: UiMessageData::Widget(WidgetMessage::TopMost),
                            destination: self.handle,
                            handled: false,
                        });

                        // Check grips.
                        for grip in self.grips.borrow_mut().iter_mut() {
                            let offset = self.screen_position;
                            let screen_bounds = grip.bounds.translate(offset.x, offset.y);
                            if screen_bounds.contains(pos.x, pos.y) {
                                dbg!(grip.kind);
                                grip.is_dragging = true;
                                self.initial_position = self.actual_local_position();
                                self.initial_size = self.actual_size();
                                self.mouse_click_pos = pos;
                                ui.capture_mouse(self.handle);
                                break;
                            }
                        }
                    }
                    WidgetMessage::MouseUp { .. } => {
                        for grip in self.grips.borrow_mut().iter_mut() {
                            if grip.is_dragging {
                                ui.release_mouse_capture();
                                grip.is_dragging = false;
                                break;
                            }
                        }
                    }
                    &WidgetMessage::MouseMove { pos, .. } => {
                        for grip in self.grips.borrow().iter() {
                            if grip.is_dragging {
                                let delta = self.mouse_click_pos - pos;
                                let (dx, dy, dw, dh) = match grip.kind {
                                    GripKind::Left => (-1.0, 0.0, 1.0, 0.0),
                                    GripKind::Top => (0.0, -1.0, 0.0, 1.0),
                                    GripKind::Right => (0.0, 0.0, -1.0, 0.0),
                                    GripKind::Bottom => (0.0, 0.0, 0.0, -1.0),
                                    GripKind::LeftTopCorner => (-1.0, -1.0, 1.0, 1.0),
                                    GripKind::RightTopCorner => (0.0, -1.0, -1.0, 1.0),
                                    GripKind::RightBottomCorner => (0.0, 0.0, -1.0, -1.0),
                                    GripKind::LeftBottomCorner => (-1.0, 0.0, 1.0, -1.0),
                                };

                                let new_pos = self.initial_position + Vec2::new(delta.x * dx, delta.y * dy);
                                let new_size= self.initial_size + Vec2::new(delta.x * dw, delta.y * dh);

                                if new_size.x > self.min_width() && new_size.x < self.max_width() &&
                                    new_size.y > self.min_height() && new_size.y < self.max_height() {
                                    self.set_desired_local_position(new_pos);
                                    self.set_width(new_size.x);
                                    self.set_height(new_size.y);
                                }

                                break;
                            }
                        }
                    }
                    _ => {}
                }

                if (message.destination == self.header || ui.node(self.header).has_descendant(message.destination, ui))
                    && !message.handled && !self.has_active_grip() {
                    match msg {
                        WidgetMessage::MouseDown { pos, .. } => {
                            message.handled = true;
                            self.mouse_click_pos = *pos;
                            ui.send_message(UiMessage {
                                handled: false,
                                data: UiMessageData::Window(WindowMessage::MoveStart),
                                destination: self.handle,
                            });
                        }
                        WidgetMessage::MouseUp { .. } => {
                            message.handled = true;
                            ui.send_message(UiMessage {
                                handled: false,
                                data: UiMessageData::Window(WindowMessage::MoveEnd),
                                destination: self.handle,
                            });
                        }
                        WidgetMessage::MouseMove { pos, .. } => {
                            if self.is_dragging {
                                self.drag_delta = *pos - self.mouse_click_pos;
                                let new_pos = self.initial_position + self.drag_delta;
                                ui.send_message(UiMessage {
                                    handled: false,
                                    data: UiMessageData::Window(WindowMessage::Move(new_pos)),
                                    destination: self.handle,
                                });
                            }
                            message.handled = true;
                        }
                        _ => ()
                    }
                }
                if let WidgetMessage::Unlink = msg {
                    if message.destination == self.handle {
                        self.initial_position = self.screen_position;
                    }
                }
            }
            UiMessageData::Button(msg) => {
                if let ButtonMessage::Click = msg {
                    if message.destination == self.minimize_button {
                        self.minimize(!self.minimized);
                    } else if message.destination == self.close_button {
                        self.close();
                    }
                }
            }
            UiMessageData::Window(msg) => {
                if message.destination == self.handle {
                    match msg {
                        WindowMessage::Open => {
                            self.set_visibility(true);
                        }
                        WindowMessage::OpenModal => {
                            if !self.visibility() {
                                self.set_visibility(true);
                                ui.push_picking_restriction(self.handle);
                            }
                        }
                        WindowMessage::Close => {
                            self.set_visibility(false);
                            ui.remove_picking_restriction(self.handle);
                        }
                        WindowMessage::Minimize(minimized) => {
                            if self.minimized != *minimized {
                                self.minimized = *minimized;
                                self.invalidate_layout();
                                if self.content.is_some() {
                                    ui.node_mut(self.content).set_visibility(!*minimized);
                                }
                            }
                        }
                        WindowMessage::CanMinimize(value) => {
                            if self.can_minimize != *value {
                                self.can_minimize = *value;
                                self.invalidate_layout();
                                if self.minimize_button.is_some() {
                                    ui.node_mut(self.minimize_button).set_visibility(*value);
                                }
                            }
                        }
                        WindowMessage::CanClose(value) => {
                            if self.can_close != *value {
                                self.can_close = *value;
                                self.invalidate_layout();
                                if self.close_button.is_some() {
                                    ui.node_mut(self.close_button).set_visibility(*value);
                                }
                            }
                        }
                        &WindowMessage::Move(new_pos) => {
                            if self.desired_local_position() != new_pos {
                                self.set_desired_local_position(new_pos);
                            }
                        }
                        WindowMessage::MoveStart => {
                            ui.capture_mouse(self.header);
                            let initial_position = self.actual_local_position();
                            self.initial_position = initial_position;
                            self.is_dragging = true;
                        }
                        WindowMessage::MoveEnd => {
                            ui.release_mouse_capture();
                            self.is_dragging = false;
                        }
                    }
                }
            }
            _ => ()
        }
    }

    fn remove_ref(&mut self, handle: Handle<UINode<M, C>>) {
        if self.header == handle {
            self.header = Handle::NONE;
        }
        if self.content == handle {
            self.content = Handle::NONE;
        }
        if self.close_button == handle {
            self.close_button = Handle::NONE;
        }
        if self.minimize_button == handle {
            self.minimize_button = Handle::NONE;
        }
    }
}

impl<M, C: 'static + Control<M, C>> Window<M, C> {
    pub fn close(&mut self) {
        self.invalidate_layout();
        self.send_message(UiMessage {
            data: UiMessageData::Window(WindowMessage::Close),
            destination: self.handle,
            handled: false,
        });
    }

    pub fn open(&mut self) {
        self.invalidate_layout();
        self.send_message(UiMessage {
            data: UiMessageData::Window(WindowMessage::Open),
            destination: self.handle,
            handled: false,
        });
    }

    pub fn minimize(&mut self, state: bool) {
        self.invalidate_layout();
        self.send_message(UiMessage {
            data: UiMessageData::Window(WindowMessage::Minimize(state)),
            destination: self.handle,
            handled: false,
        });
    }

    pub fn set_can_close(&mut self, state: bool) {
        self.invalidate_layout();
        self.send_message(UiMessage {
            data: UiMessageData::Window(WindowMessage::CanClose(state)),
            destination: self.handle,
            handled: false,
        });
    }

    pub fn set_can_minimize(&mut self, state: bool) {
        self.invalidate_layout();
        self.send_message(UiMessage {
            data: UiMessageData::Window(WindowMessage::CanMinimize(state)),
            destination: self.handle,
            handled: false,
        });
    }

    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }

    pub fn drag_delta(&self) -> Vec2 {
        self.drag_delta
    }

    pub fn has_active_grip(&self) -> bool {
        for grip in self.grips.borrow().iter() {
            if grip.is_dragging {
                return true;
            }
        }
        false
    }
}

pub struct WindowBuilder<'a, M: 'static, C: 'static + Control<M, C>> {
    widget_builder: WidgetBuilder<M, C>,
    content: Handle<UINode<M, C>>,
    title: Option<WindowTitle<'a, M, C>>,
    can_close: bool,
    can_minimize: bool,
    open: bool,
    close_button: Option<Handle<UINode<M, C>>>,
    minimize_button: Option<Handle<UINode<M, C>>>,
    modal: bool
}

/// Window title can be either text or node.
///
/// If `Text` is used, then builder will automatically create Text node with specified text,
/// but with default font.
///
/// If you need more flexibility (i.e. put a picture near text) then `Node` option is for you:
/// it allows to put any UI node hierarchy you want to.
pub enum WindowTitle<'a, M: 'static, C: 'static + Control<M, C>> {
    Text(&'a str),
    Node(Handle<UINode<M, C>>),
}

impl<'a, M, C: 'static + Control<M, C>> WindowBuilder<'a, M, C> {
    pub fn new(widget_builder: WidgetBuilder<M, C>) -> Self {
        Self {
            widget_builder,
            content: Handle::NONE,
            title: None,
            can_close: true,
            can_minimize: true,
            open: true,
            close_button: None,
            minimize_button: None,
            modal: false
        }
    }

    pub fn with_content(mut self, content: Handle<UINode<M, C>>) -> Self {
        self.content = content;
        self
    }

    pub fn with_title(mut self, title: WindowTitle<'a, M, C>) -> Self {
        self.title = Some(title);
        self
    }

    pub fn with_minimize_button(mut self, button: Handle<UINode<M, C>>) -> Self {
        self.minimize_button = Some(button);
        self
    }

    pub fn with_close_button(mut self, button: Handle<UINode<M, C>>) -> Self {
        self.close_button = Some(button);
        self
    }

    pub fn can_close(mut self, can_close: bool) -> Self {
        self.can_close = can_close;
        self
    }

    pub fn can_minimize(mut self, can_minimize: bool) -> Self {
        self.can_minimize = can_minimize;
        self
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    pub fn build(self, ui: &mut UserInterface<M, C>) -> Handle<UINode<M, C>> {
        let minimize_button;
        let close_button;

        let header = BorderBuilder::new(WidgetBuilder::new()
            .with_horizontal_alignment(HorizontalAlignment::Stretch)
            .with_height(30.0)
            .with_background(Brush::LinearGradient {
                from: Vec2::new(0.5, 0.0),
                to: Vec2::new(0.5, 1.0),
                stops: vec![
                    GradientPoint { stop: 0.0, color: Color::opaque(85, 85, 85) },
                    GradientPoint { stop: 0.5, color: Color::opaque(65, 65, 65) },
                    GradientPoint { stop: 1.0, color: Color::opaque(75, 75, 75) },
                ],
            })
            .with_child(GridBuilder::new(WidgetBuilder::new()
                .with_child({
                    match self.title {
                        None => Handle::NONE,
                        Some(window_title) => {
                            match window_title {
                                WindowTitle::Node(node) => node,
                                WindowTitle::Text(text) => {
                                    TextBuilder::new(WidgetBuilder::new()
                                        .with_margin(Thickness::uniform(5.0))
                                        .on_row(0)
                                        .on_column(0))
                                        .with_text(text)
                                        .build(ui)
                                }
                            }
                        }
                    }
                })
                .with_child({
                    minimize_button = self.minimize_button.unwrap_or_else(|| {
                        ButtonBuilder::new(WidgetBuilder::new()
                            .with_margin(Thickness::uniform(2.0)))
                            .with_text("_")
                            .build(ui)
                    });
                    ui.node_mut(minimize_button)
                        .set_visibility(self.can_minimize)
                        .set_width_mut(30.0)
                        .set_row(0)
                        .set_column(1);
                    minimize_button
                })
                .with_child({
                    close_button = self.close_button.unwrap_or_else(|| {
                        ButtonBuilder::new(WidgetBuilder::new()
                            .with_margin(Thickness::uniform(2.0)))
                            .with_text("X")
                            .build(ui)
                    });
                    ui.node_mut(close_button)
                        .set_width_mut(30.0)
                        .set_visibility(self.can_close)
                        .set_row(0)
                        .set_column(2);
                    close_button
                }))
                .add_column(Column::stretch())
                .add_column(Column::auto())
                .add_column(Column::auto())
                .add_row(Row::stretch())
                .build(ui))
            .on_row(0)
        ).build(ui);

        ui.node_mut(self.content).set_row(1);

        let window = Window {
            widget: self.widget_builder
                .with_visibility(self.open)
                .with_child(BorderBuilder::new(WidgetBuilder::new()
                    .with_child(GridBuilder::new(WidgetBuilder::new()
                        .with_child(self.content)
                        .with_child(header))
                        .add_column(Column::stretch())
                        .add_row(Row::auto())
                        .add_row(Row::stretch())
                        .build(ui)))
                    .build(ui))
                .build(ui.sender()),
            mouse_click_pos: Vec2::ZERO,
            initial_position: Vec2::ZERO,
            initial_size: Default::default(),
            is_dragging: false,
            minimized: false,
            can_minimize: self.can_minimize,
            can_close: self.can_close,
            header,
            minimize_button,
            close_button,
            drag_delta: Default::default(),
            content: self.content,
            grips: RefCell::new([
                // Corners have priority
                Grip::new(GripKind::LeftTopCorner),
                Grip::new(GripKind::RightTopCorner),
                Grip::new(GripKind::RightBottomCorner),
                Grip::new(GripKind::LeftBottomCorner),
                Grip::new(GripKind::Left),
                Grip::new(GripKind::Top),
                Grip::new(GripKind::Right),
                Grip::new(GripKind::Bottom),
            ]),
        };

        let handle = ui.add_node(UINode::Window(window));

        ui.flush_messages();

        if self.modal {
            ui.push_picking_restriction(handle);
        }

        handle
    }
}