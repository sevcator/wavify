//! A long list that builds only the rows on screen (Liked Tracks, a big
//! playlist, the queue). iced rebuilds and lays out the whole view after
//! every message, and a thousand track rows made each of those take tens of
//! milliseconds.
//!
//! The app builds rows `first..last` between two spacers as tall as the
//! rows left out, so the list keeps its full height and every row its place:
//! the scroll position never jumps. This widget watches where the list sits
//! in the scrolled view and asks for another window (`on_window`) once the
//! visible rows run past the built ones.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::event::{self, Event};
use iced::widget::{Column, Space};
use iced::{mouse, Element, Length, Rectangle, Renderer, Size, Theme, Vector};

/// Rows built past the visible ones, above and below: a few wheel notches
/// never reach rows that aren't there yet.
const OVERSCAN: usize = 16;

pub struct VirtualList<'a, Message> {
    content: Element<'a, Message>,
    /// Row height plus the spacing between rows.
    pitch: f32,
    count: usize,
    built: (usize, usize),
    on_window: Box<dyn Fn(usize, usize) -> Message + 'a>,
}

/// `rows` are rows `built.0..built.1` of a list of `count`, `pitch` apart
/// (their height plus `spacing`); `on_window(first, last)` asks for the rows
/// to build next.
pub fn virtual_list<'a, Message: 'a>(
    count: usize,
    pitch: f32,
    spacing: f32,
    built: (usize, usize),
    rows: Vec<Element<'a, Message>>,
    on_window: impl Fn(usize, usize) -> Message + 'a,
) -> VirtualList<'a, Message> {
    let above = built.0 as f32 * pitch;
    let below = count.saturating_sub(built.1) as f32 * pitch;
    let content = Column::new()
        .push(Space::new(Length::Fixed(0.0), Length::Fixed(above)))
        .push(Column::with_children(rows).spacing(spacing))
        .push(Space::new(Length::Fixed(0.0), Length::Fixed(below)))
        .into();
    VirtualList {
        content,
        pitch: pitch.max(1.0),
        count,
        built,
        on_window: Box::new(on_window),
    }
}

impl<'a, Message: 'a> Widget<Message, Theme, Renderer> for VirtualList<'a, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let child = self
            .content
            .as_widget()
            .layout(&mut tree.children[0], renderer, limits);
        layout::Node::with_children(child.size(), vec![child])
    }

    fn operate(
        &self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content.as_widget().operate(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            operation,
        );
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
        let status = self.content.as_widget_mut().on_event(
            &mut tree.children[0],
            event,
            layout.children().next().unwrap(),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        // The viewport is the scrolled view's visible part, in the list's
        // coordinates: the rows it covers must be built.
        let bounds = layout.bounds();
        let top = viewport.y - bounds.y;
        let bottom = top + viewport.height;
        if self.count > 0 && bottom > 0.0 && top < bounds.height {
            let first = (top.max(0.0) / self.pitch) as usize;
            let last = ((bottom.min(bounds.height) / self.pitch).ceil() as usize).min(self.count);
            if first < self.built.0 || last > self.built.1 {
                let want = (
                    first.saturating_sub(OVERSCAN),
                    (last + OVERSCAN).min(self.count),
                );
                if want != self.built {
                    shell.publish((self.on_window)(want.0, want.1));
                }
            }
        }
        status
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout.children().next().unwrap(),
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout.children().next().unwrap(),
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout.children().next().unwrap(),
            renderer,
            translation,
        )
    }
}

impl<'a, Message: 'a> From<VirtualList<'a, Message>> for Element<'a, Message> {
    fn from(list: VirtualList<'a, Message>) -> Self {
        Element::new(list)
    }
}
