// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2023 Jude Nelson
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::io::{Write, stdout, stdin, Stdin, Stdout, Error as IOError};
use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::fs;
use std::io;
use std::io::Read;

use termion::async_stdin;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::IntoAlternateScreen;

use crate::ui::root::Root;
use crate::ui::root::FrameUpdate;
use crate::ui::Renderer;
use crate::ui::events::WrbUIEventChannels;
use crate::ui::events::WrbFrameData;
use crate::ui::forms::WrbFormEvent;
use crate::ui::events::WrbEvent;
use crate::ui::scanline::Scanline;
use crate::ui::Error as UIError;

pub mod status;

use crate::viewer::status::ViewerStatus;

use stacks_common::util::sleep_ms;

#[derive(Clone, Debug, PartialEq)]
pub enum ViewerFocus {
    /// No focus (command mode)
    NoFocus,
    /// focus is on the status widget
    Status,
    /// focus is somewhere in the root
    Root
}

#[derive(Clone, Debug)]
pub enum ViewerEvent {
    Stdin(Key),
    Root(Root),
    Update(FrameUpdate),
}

pub struct Viewer {
    /// row, column dimensions
    size: (u64, u64),
    /// location of the cursor
    cursor: (u16, u16),
    /// frames in, events out
    events: Option<WrbUIEventChannels>,
    /// last frame we got from the wrbpage main loop
    last_frame: Option<Root>,
    /// status widget
    status: ViewerStatus,
    /// where input focus is
    focus: ViewerFocus,
    /// whether or not to abort the main loop
    quit: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum Error {
    IO(IOError),
    UI(UIError),
    Finished
}

impl From<IOError> for Error {
    fn from(e: IOError) -> Self {
        Self::IO(e)
    }
}

impl From<UIError> for Error {
    fn from(e: UIError) -> Self {
        Self::UI(e)
    }
}

impl Viewer {
    pub fn new(events: WrbUIEventChannels, wrbname: &str) -> Self {
        Self {
            size: (0, 0),
            cursor: (0, 0),
            events: Some(events),
            last_frame: None,
            status: ViewerStatus::new(wrbname.to_string(), false),
            focus: ViewerFocus::NoFocus,
            quit: Arc::new(AtomicBool::new(false)),
        }
    }

    /// cursor goto
    fn goto_cursor(&self) -> String {
        format!("{}", termion::cursor::Goto(self.cursor.0.saturating_add(1), self.cursor.1.saturating_add(1)))
    }

    /// hide the cursor
    fn hide_cursor<W: Write>(&mut self, stdout: &mut W) -> Result<(), Error> {
        write!(stdout, "{}{}", termion::cursor::Goto(1, 1), termion::cursor::Hide)?;
        self.cursor = (1, 1);
        Ok(())
    }
    
    /// show the cursor
    fn show_cursor<W: Write>(&mut self, stdout: &mut W) -> Result<(), Error> {
        write!(stdout, "{}", termion::cursor::Show)?;
        Ok(())
    }
    
    /// clear the whole screen
    fn clear_screen<W: Write>(&mut self, stdout: &mut W) -> Result<(), Error> {
        write!(stdout, "{}", termion::clear::All)?;
        Ok(())
    }

    /// get terminal size
    fn get_term_size(&mut self) -> Result<(u64, u64), Error> {
        let (cols, rows) = termion::terminal_size()?;
        Ok((u64::from(rows), u64::from(cols)))
    }

    /// get root size
    fn get_root_size(&mut self, term_rows: u64, term_cols: u64) -> (u64, u64) {
        (term_rows.saturating_sub(self.status.num_rows()), term_cols)
    }

    /// no focus
    fn set_no_focus<W: Write>(&mut self, stdout: &mut W) -> Result<(), Error> {
        self.focus = ViewerFocus::NoFocus;
        self.hide_cursor(stdout)?;
        Ok(())
    }
    
    /// status bar focus
    fn set_status_focus<W: Write>(&mut self, stdout: &mut W) -> Result<(), Error> {
        self.focus = ViewerFocus::Status;
        let status_row = if self.status.at_top() {
            1
        }
        else {
            u16::try_from(self.size.0.saturating_sub(self.status.num_rows())).unwrap_or(u16::MAX - 1)
        };

        self.cursor = (u16::try_from(self.status.cursor_column(true)).unwrap_or(u16::MAX - 1), status_row);
        self.show_cursor(stdout)?;
        Ok(())
    }

    /// Set the quit flag and set quit status text
    fn set_quit(&mut self) {
        self.quit.store(true, Ordering::SeqCst);
        self.status.set_text("Done! Press any key to exit".to_string());
    }

    pub fn dispatch_keyboard_event<W: Write>(&mut self, key: Key, frame: Option<&mut Root>, stdout: &mut W) -> Result<(), Error> {
        wrb_debug!("Got key in focus {:?}: {:?}", &self.focus, &key);
        
        // if we have no frame, then focus reverts to the Status widget
        if frame.is_none() {
            self.focus = ViewerFocus::Status;
            self.set_status_focus(stdout)?;
        }

        match self.focus {
            ViewerFocus::NoFocus => {
                // interpret keys as commands
                match key {
                    Key::Char('g') => {
                        self.set_status_focus(stdout)?;
                    }
                    Key::Char('\t') => {
                        self.focus = ViewerFocus::Root;
                        if let Some(frame) = frame {
                            frame.next_focus();
                        }
                    }
                    Key::Char('\n') => {
                        self.focus = ViewerFocus::Root;
                    }
                    Key::Char('q') => {
                        self.set_quit();
                    }
                    _ => {}
                }
            }
            ViewerFocus::Status => {
                match key {
                    Key::Esc => {
                        self.set_no_focus(stdout)?;
                    }
                    Key::Char('\t') => {
                        self.focus = ViewerFocus::Root;
                        if let Some(frame) = frame {
                            frame.next_focus();
                        }
                    }
                    _ => {
                        self.status.handle_event(WrbFormEvent::Keypress(key))?;
                        self.set_status_focus(stdout)?;
                    }
                }
            },
            ViewerFocus::Root => {
                match key {
                    Key::Esc => {
                        self.set_no_focus(stdout)?;
                        if let Some(frame) = frame {
                            frame.clear_focus();
                        }
                    }
                    Key::Char('\t') => {
                        if let Some(frame) = frame {
                            frame.next_focus();
                        }
                    }
                    _ => {
                        if let Some(frame) = frame {
                            frame.handle_event(WrbFormEvent::Keypress(key))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Keyboard reader thread
    fn start_keyboard_thread(quit: Arc<AtomicBool>, key_sender: Sender<ViewerEvent>) -> JoinHandle<()> {
        let stdin = stdin();
        let handle = thread::spawn(move || {
            for c in stdin.keys() {
                let Ok(k) = c else {
                    return;
                };
                if key_sender.send(ViewerEvent::Stdin(k)).is_err() {
                    return;
                }
                if quit.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
        handle
    }

    /// Start frame thread
    fn start_frame_thread(quit: Arc<AtomicBool>, frame_receiver: Receiver<WrbFrameData>, frame_sender: Sender<ViewerEvent>) -> JoinHandle<()> {
        let handle = thread::spawn(move || {
            while !quit.load(Ordering::SeqCst) {
                let Ok(frame_data) = frame_receiver.recv() else {
                    return;
                };
                match frame_data {
                    WrbFrameData::Root(root) => {
                        if frame_sender.send(ViewerEvent::Root(root)).is_err() {
                            return;
                        }
                    }
                    WrbFrameData::Update(update) => {
                        if frame_sender.send(ViewerEvent::Update(update)).is_err() {
                            return;
                        }
                    }
                }
            }
        });
        handle
    }

    /// Render a frame. Saves it to last_frame
    fn render<W: Write>(&mut self, mut root: Root, screen: &mut W) -> Result<(), Error> {
        let status_text = self.status.render(self.focus == ViewerFocus::Status, self.size.1);
        let (root_rows, _) = self.get_root_size(self.size.0, self.size.1);
        let root_text = {
            let chars = root.render();
            let scanlines = Scanline::compile_rows(&chars, 0, root_rows);
            Renderer::scanlines_into_term_string(scanlines)
        };
        
        self.last_frame = Some(root);

        if self.status.at_top() {
            write!(screen, "{}{}{}{}", termion::cursor::Goto(1, 1), &status_text, &root_text, &self.goto_cursor())?;
        }
        else {
            let status_row = u16::try_from(self.size.0.saturating_sub(self.status.num_rows())).unwrap_or(u16::MAX - 1);
            write!(screen, "{}{}{}{}{}", termion::cursor::Goto(1, 1), &root_text, termion::cursor::Goto(1, status_row + 1), &status_text, &self.goto_cursor())?;
        };
        
        screen.flush()?;
        Ok(())
    }

    /// Main event loop
    pub fn main(mut self) -> Result<(), Error> {
        let mut screen = stdout().lock().into_raw_mode()?.into_alternate_screen()?;

        self.clear_screen(&mut screen)?;
        self.hide_cursor(&mut screen)?;

        let Some(events) = self.events.take() else {
            return Ok(());
        };

        let (events_send, frames_recv) = events.destruct();
        let (viewer_send, viewer_recv) = channel();

        let keyboard_thread = Self::start_keyboard_thread(self.quit.clone(), viewer_send.clone());
        let frame_thread = Self::start_frame_thread(self.quit.clone(), frames_recv, viewer_send.clone());

        // request the page to be generated
        if events_send.send(WrbEvent::Open).is_err() {
            self.set_quit();
        }

        let mut timer_thread = None;

        while !self.quit.load(Ordering::SeqCst) {
            let sz = self.get_term_size()?;
            if sz != self.size {
                let (root_rows, root_cols) = self.get_root_size(sz.0, sz.1);
                if events_send.send(WrbEvent::Resize(root_rows, root_cols)).is_err() { 
                    self.set_quit();
                }

                self.size = sz;
            }

            match viewer_recv.recv() {
                Ok(ViewerEvent::Stdin(key)) => {
                    let mut last_frame = self.last_frame.take();
                    self.dispatch_keyboard_event(key, last_frame.as_mut(), &mut screen)?;

                    if let Some(mut frame) = last_frame {
                        frame.redraw()?;
                        self.render(frame, &mut screen)?;
                    }
                }
                Ok(ViewerEvent::Root(root)) => {
                    let frame_delay_opt = root.frame_delay.clone();

                    // start feeding the event loop timer events, now that we know the delay
                    if let Some(frame_delay) = frame_delay_opt {
                        if timer_thread.is_none() {
                            // begin sending wakeup events to the main loop
                            let event_sender = events_send.clone();
                            timer_thread = Some(thread::spawn(move || {
                                loop {
                                    if event_sender.send(WrbEvent::Timer).is_err() {
                                        break;
                                    }
                                    sleep_ms(frame_delay);
                                }
                            }));
                        }
                    }

                    self.render(root, &mut screen)?;
                }
                Ok(ViewerEvent::Update(update)) => {
                    if let Some(mut last_frame) = self.last_frame.take() {
                        last_frame.update_forms(update)?;
                        self.render(last_frame, &mut screen)?;
                    }
                }
                Err(e) => {
                    wrb_debug!("Exiting viewer loop: {:?}", &e);
                    self.set_quit();
                }
            }
        }

        self.show_cursor(&mut screen)?;
        wrb_debug!("Viewer main exit");

        let _ = events_send.send(WrbEvent::Close);

        if let Some(timer_thread) = timer_thread.take() {
            let _ = timer_thread.join();
        }
        let _ = keyboard_thread.join();
        let _ = frame_thread.join();

        Ok(())
    }
}

