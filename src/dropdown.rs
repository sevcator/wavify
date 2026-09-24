//! A menu that drops down from the widget that opened it (Spotify's
//! context menus), instead of a side panel. The menu is an overlay placed
//! at the anchor's on-screen position, so it follows the anchor inside a
//! scrolled list, and flips above / shifts left when there's no room.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::event::{self, Event};
use iced::{mouse, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector};

pub struct Dropdown<'a, Message> {
    anchor: Element<'a, Message>,
    /// The open menu; a zero-size placeholder while closed, so the widget
    /// tree keeps the same shape either way.
    menu: Element<'a, Message>,
    open: bool,
    /// Sent on a click outside the open menu.
    on_dismiss: Option<Message>,
    /// Space between the anchor and the menu.
    gap: f32,
}

/// `anchor`, with `menu` dropped down from it while `Some`.
pub fn dropdown<'a, Message: Clone + 'a>(
    anchor: impl Into<Element<'a, Message>>,
    menu: Option<Element<'a, Message>>,
) -> Dropdown<'a, Message> {
    let open = menu.is_some();
    Dropdown {
        anchor: anchor.into(),
        menu: menu.unwrap_or_else(|| {
            iced::widget::Space::new(Length::Fixed(0.0), Length::Fixed(0.0)).into()
        }),
        open,
        on_dismiss: None,
        gap: 6.0,
    }
}

impl<'a, Message: Clone + 'a> Dropdown<'a, Message> {
    pub fn on_dismiss(mut self, message: Message) -> Self {
        self.on_dismiss = Some(message);
        self
    }
}

impl<'a, Message: Clone + 'a> Widget<Message, Theme, Renderer> for Dropdown<'a, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.anchor), Tree::new(&self.menu)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[self.anchor.as_widget(), self.menu.as_widget()]);
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.anchor
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.anchor
            .as_widget()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) -> event::Status {
        self.anchor.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.anchor.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.anchor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let mut children = tree.children.iter_mut();
        let anchor_overlay = self.anchor.as_widget_mut().overlay(
            children.next().unwrap(),
            layout,
            renderer,
            translation,
        );
        let menu_overlay = self.open.then(|| {
            overlay::Element::new(Box::new(Menu {
                anchor: Rectangle::new(layout.position() + translation, layout.bounds().size()),
                menu: &mut self.menu,
                state: children.next().unwrap(),
                on_dismiss: self.on_dismiss.clone(),
                gap: self.gap,
            }))
        });
        let all: Vec<_> = anchor_overlay.into_iter().chain(menu_overlay).collect();
        (!all.is_empty()).then(|| overlay::Group::with_children(all).overlay())
    }
}

impl<'a, Message: Clone + 'a> From<Dropdown<'a, Message>> for Element<'a, Message> {
    fn from(d: Dropdown<'a, Message>) -> Self {
        Element::new(d)
    }
}

struct Menu<'a, 'b, Message> {
    /// The anchor's bounds on screen.
    anchor: Rectangle,
    menu: &'b mut Element<'a, Message>,
    state: &'b mut Tree,
    on_dismiss: Option<Message>,
    gap: f32,
}

impl<'a, 'b, Message: Clone> overlay::Overlay<Message, Theme, Renderer> for Menu<'a, 'b, Message> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let node = self.menu.as_widget().layout(
            self.state,
            renderer,
            &layout::Limits::new(Size::ZERO, bounds),
        );
        let size = node.size();
        // Left edges together, like Spotify; slid left when it would run
        // off the window's right edge (a "..." at the end of a row).
        let x = self.anchor.x.min(bounds.width - size.width).max(0.0);
        // Below the anchor; above it when there's no room below.
        let below = self.anchor.y + self.anchor.height + self.gap;
        let above = self.anchor.y - self.gap - size.height;
        let y = if below + size.height <= bounds.height || above < 0.0 {
            below.min(bounds.height - size.height).max(0.0)
        } else {
            above
        };
        node.move_to(Point::new(x, y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        self.menu
            .as_widget()
            .draw(self.state, renderer, theme, style, layout, cursor, &bounds);
    }

    fn on_event(
        &mut self,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        let bounds = layout.bounds();
        // A press outside closes the menu and goes no further, like a
        // native menu: the click that dismisses doesn't also press whatever
        // was under it (the "..." that opened it included).
        if let Event::Mouse(mouse::Event::ButtonPressed(_)) = event {
            if !cursor.is_over(bounds) {
                if let Some(m) = &self.on_dismiss {
                    shell.publish(m.clone());
                }
                return event::Status::Captured;
            }
        }
        self.menu.as_widget_mut().on_event(
            self.state, event, layout, cursor, renderer, clipboard, shell, &bounds,
        )
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.menu
            .as_widget()
            .mouse_interaction(self.state, layout, cursor, viewport, renderer)
    }
}
