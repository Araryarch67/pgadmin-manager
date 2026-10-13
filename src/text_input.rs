//! TextInput satu baris ala contoh resmi GPUI (`input.rs`), disederhanakan:
//! - Kursor/seleksi disimpan sebagai offset CHAR (bukan byte) agar pemetaan
//!   ke string tampilan (masking password 1 char -> 1 char) selalu tepat.
//! - Tinggi fixed 34px, gaya Everblush, focus ring aksen.
//! - Shortcut umum via actions + bind_keys di main.rs:
//!   panah, shift+panah (seleksi), Ctrl+A/C/X/V, Home/End,
//!   Backspace/Delete, klik + drag seleksi, paste clipboard.

use std::ops::Range;

use gpui::prelude::*;
use gpui::{
    div, fill, point, px, relative, size, App, Bounds, ClipboardItem, Context, CursorStyle,
    Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle, Focusable,
    GlobalElementId, Hsla, InspectorElementId, InteractiveElement, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, Render,
    ShapedLine, SharedString, Style, Styled, TextRun, UTF16Selection, UnderlineStyle, Window,
};

use crate::{accent, muted};
use crate::{
    Backspace, Copy, Cut, Delete, End, Home, Left, Paste, Right, SelectAll, SelectLeft, SelectRight,
};

pub struct TextInput {
    element_id: &'static str,
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    password: bool,
    cursor: usize,         // offset char
    anchor: Option<usize>, // offset char awal seleksi; None = tidak ada seleksi
    marked: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    is_selecting: bool,
}

impl TextInput {
    pub fn new(
        cx: &mut Context<Self>,
        id: &'static str,
        initial: &str,
        placeholder: &str,
        password: bool,
    ) -> Self {
        Self {
            element_id: id,
            focus_handle: cx.focus_handle(),
            content: initial.to_string().into(),
            placeholder: placeholder.to_string().into(),
            password,
            cursor: initial.chars().count(),
            anchor: None,
            marked: None,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            is_selecting: false,
        }
    }

    pub fn text(&self) -> String {
        self.content.to_string()
    }

    /// String yang digambar. Masking 1 char -> 1 char sehingga offset char
    /// pada display selalu sama dengan offset char pada content.
    fn display(&self) -> String {
        if self.password {
            "•".repeat(self.content.chars().count())
        } else {
            self.content.to_string()
        }
    }

    fn char_len(&self) -> usize {
        self.content.chars().count()
    }

    fn selection(&self) -> Option<Range<usize>> {
        self.anchor.map(|a| {
            if a <= self.cursor {
                a..self.cursor
            } else {
                self.cursor..a
            }
        })
    }

    fn move_to(&mut self, ci: usize, cx: &mut Context<Self>) {
        self.cursor = ci.min(self.char_len());
        self.anchor = None;
        cx.notify();
    }

    fn select_to(&mut self, ci: usize, cx: &mut Context<Self>) {
        let ci = ci.min(self.char_len());
        if self.anchor.is_none() {
            self.anchor = Some(self.cursor);
        }
        self.cursor = ci;
        if self.anchor == Some(self.cursor) {
            self.anchor = None;
        }
        cx.notify();
    }

    fn replace_char_range(&mut self, range: Range<usize>, text: &str) {
        let clean: String = text.chars().filter(|c| *c != '\n').collect();
        let mut chars: Vec<char> = self.content.chars().collect();
        chars.splice(range.clone(), clean.chars());
        self.content = chars.into_iter().collect::<String>().into();
        self.cursor = range.start + clean.chars().count();
        self.anchor = None;
        self.marked = None;
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection().is_some() {
            let s = self.selection().unwrap();
            self.move_to(s.start, cx);
        } else if self.cursor > 0 {
            self.move_to(self.cursor - 1, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection().is_some() {
            let s = self.selection().unwrap();
            self.move_to(s.end, cx);
        } else {
            self.move_to(self.cursor + 1, cx);
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.cursor.saturating_sub(1), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.cursor + 1, cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.anchor = Some(0);
        self.cursor = self.char_len();
        cx.notify();
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.char_len(), cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(sel) = self.selection() {
            self.replace_char_range(sel, "");
        } else if self.cursor > 0 {
            self.replace_char_range(self.cursor - 1..self.cursor, "");
        }
        cx.notify();
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(sel) = self.selection() {
            self.replace_char_range(sel, "");
        } else if self.cursor < self.char_len() {
            self.replace_char_range(self.cursor..self.cursor + 1, "");
        }
        cx.notify();
    }

    fn selected_text(&self) -> Option<String> {
        self.selection().map(|sel| {
            let (s, e) = (
                byte_idx(&self.content, sel.start),
                byte_idx(&self.content, sel.end),
            );
            self.content[s..e].to_string()
        })
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            if let Some(sel) = self.selection() {
                self.replace_char_range(sel, &text);
            } else {
                let at = self.cursor;
                self.replace_char_range(at..at, &text);
            }
            cx.notify();
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.selected_text() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            if let Some(sel) = self.selection() {
                self.replace_char_range(sel, "");
            }
            cx.notify();
        }
    }

    fn index_for_mouse(&self, position: Point<Pixels>) -> usize {
        let display = self.display();
        if display.is_empty() {
            return 0;
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return self.char_len();
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.char_len();
        }
        let bi = line.index_for_x(position.x - bounds.left() + self.scroll_x);
        byte_to_char(&display, bi.unwrap_or(display.len()))
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_selecting = true;
        let idx = self.index_for_mouse(event.position);
        if event.modifiers.shift {
            self.select_to(idx, cx);
        } else {
            self.move_to(idx, cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse(event.position), cx);
        }
    }

    // ---- konversi offset (byte <-> char <-> utf16) ----
    fn offset_to_utf16(&self, byte_off: usize) -> usize {
        let mut u16off = 0;
        let mut count = 0;
        for ch in self.content.chars() {
            if count >= byte_off {
                break;
            }
            count += ch.len_utf8();
            u16off += ch.len_utf16();
        }
        u16off
    }

    fn offset_from_utf16(&self, u16off: usize) -> usize {
        let mut byte_off = 0;
        let mut count = 0;
        for ch in self.content.chars() {
            if count >= u16off {
                break;
            }
            count += ch.len_utf16();
            byte_off += ch.len_utf8();
        }
        byte_off
    }

    fn chars_of_bytes(&self, range: Range<usize>) -> Range<usize> {
        byte_to_char(&self.content, range.start)..byte_to_char(&self.content, range.end)
    }

    fn bytes_of_chars(&self, range: Range<usize>) -> Range<usize> {
        byte_idx(&self.content, range.start)..byte_idx(&self.content, range.end)
    }
}

fn byte_idx(s: &str, ci: usize) -> usize {
    s.char_indices().nth(ci).map(|(b, _)| b).unwrap_or(s.len())
}

fn byte_to_char(s: &str, bi: usize) -> usize {
    s.char_indices().take_while(|(b, _)| *b < bi).count()
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let start = self.offset_from_utf16(range_utf16.start);
        let end = self.offset_from_utf16(range_utf16.end);
        actual_range.replace(self.offset_to_utf16(start)..self.offset_to_utf16(end));
        Some(self.content[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (a, c) = (self.anchor.unwrap_or(self.cursor), self.cursor);
        let lo = a.min(c);
        let hi = a.max(c);
        let blo = byte_idx(&self.content, lo);
        let bhi = byte_idx(&self.content, hi);
        Some(UTF16Selection {
            range: self.offset_to_utf16(blo)..self.offset_to_utf16(bhi),
            reversed: a > c,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked.as_ref().map(|m| {
            let b = self.bytes_of_chars(m.clone());
            self.offset_to_utf16(b.start)..self.offset_to_utf16(b.end)
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let byte_range = range_utf16
            .map(|r| self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end))
            .or_else(|| self.marked.clone().map(|m| self.bytes_of_chars(m)))
            .or_else(|| self.selection().map(|s| self.bytes_of_chars(s)))
            .unwrap_or_else(|| {
                let b = byte_idx(&self.content, self.cursor);
                b..b
            });
        let char_range = self.chars_of_bytes(byte_range);
        self.replace_char_range(char_range, new_text);
        self.marked = None;
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let byte_range = range_utf16
            .map(|r| self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end))
            .or_else(|| self.marked.clone().map(|m| self.bytes_of_chars(m)))
            .or_else(|| self.selection().map(|s| self.bytes_of_chars(s)))
            .unwrap_or_else(|| {
                let b = byte_idx(&self.content, self.cursor);
                b..b
            });
        let char_range = self.chars_of_bytes(byte_range);
        let start = char_range.start;
        self.replace_char_range(char_range, new_text);
        let inserted = new_text.chars().count();
        if inserted > 0 {
            self.marked = Some(start..start + inserted);
        }
        // Kursor selalu di akhir teks komposisi (seleksi bawaan IME diabaikan).
        self.cursor = start + inserted;
        self.anchor = None;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let display = self.display();
        let s = self.offset_from_utf16(range_utf16.start);
        let e = self.offset_from_utf16(range_utf16.end);
        let s = byte_to_char(&self.content, s);
        let e = byte_to_char(&self.content, e);
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(byte_idx(&display, s)) - self.scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(byte_idx(&display, e)) - self.scroll_x,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let display = self.display();
        let bi = last_layout.index_for_x(point.x - line_point.x + self.scroll_x)?;
        let ci = byte_to_char(&display, bi);
        let content_bytes = byte_idx(&self.content, ci.min(self.char_len()));
        Some(self.offset_to_utf16(content_bytes))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let display = input.display();
        let sel_chars = input.selection();
        let cursor_chars = input.cursor;
        let scroll = input.scroll_x;
        let style = window.text_style();

        let (display_text, text_color) = if display.is_empty() {
            (input.placeholder.clone(), Hsla::from(muted()))
        } else {
            (display.clone().into(), style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let marked_bytes: Option<Range<usize>> = input.marked.clone().map(|m| {
            byte_idx(&display, m.start.min(display.chars().count()))
                ..byte_idx(&display, m.end.min(display.chars().count()))
        });
        let runs = if let Some(mr) = marked_bytes {
            vec![
                TextRun {
                    len: mr.start,
                    ..run.clone()
                },
                TextRun {
                    len: mr.end.saturating_sub(mr.start),
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len().saturating_sub(mr.end),
                    ..run
                },
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        let sel_bytes = sel_chars.map(|s| {
            byte_idx(&display, s.start.min(display.chars().count()))
                ..byte_idx(&display, s.end.min(display.chars().count()))
        });
        let cursor_bytes = byte_idx(&display, cursor_chars.min(display.chars().count()));

        // Ikuti kursor agar selalu terlihat (scroll horizontal).
        let cursor_x = line.x_for_index(cursor_bytes);
        let view_w = bounds.size.width;
        let mut new_scroll = scroll;
        if cursor_x - scroll > view_w {
            new_scroll = cursor_x - view_w + px(8.);
        }
        if cursor_x < scroll {
            new_scroll = cursor_x;
        }
        let max_scroll = (line.width - view_w + px(8.)).max(px(0.));
        new_scroll = new_scroll.clamp(px(0.), max_scroll.max(px(0.)));

        let shift = new_scroll;
        let (selection, cursor) = match sel_bytes {
            Some(r) if !r.is_empty() => (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(r.start) - shift,
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(r.end) - shift,
                            bounds.bottom(),
                        ),
                    ),
                    Hsla {
                        a: 0.28,
                        ..Hsla::from(accent())
                    },
                )),
                None,
            ),
            _ => (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_x - shift, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    Hsla::from(accent()),
                )),
            ),
        };

        self.input.update(cx, |input, _| {
            input.scroll_x = new_scroll;
            input.last_layout = Some(line.clone());
            input.last_bounds = Some(bounds);
        });

        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let scroll = self.input.read(cx).scroll_x;
        line.paint(
            point(bounds.left() - scroll, bounds.top()),
            window.line_height(),
            window,
            cx,
        )
        .unwrap();

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }
    }
}

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        div()
            .id(self.element_id)
            .flex()
            .items_center()
            .key_context("TextInput")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .h(px(34.))
            .w_full()
            .px_3()
            .rounded_md()
            .overflow_hidden()
            .bg(crate::bg_field())
            .border_1()
            .border_color(if focused {
                crate::accent()
            } else {
                crate::border()
            })
            .text_sm()
            .child(TextElement { input: cx.entity() })
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
