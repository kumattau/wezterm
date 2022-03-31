use crate::termwindow::TermWindowNotif;
use crate::TermWindow;
use config::keyassignment::{ClipboardCopyDestination, ClipboardPasteSource};
use mux::pane::Pane;
use mux::window::WindowId as MuxWindowId;
use mux::Mux;
use std::rc::Rc;
use std::sync::Arc;
use window::{Clipboard, WindowOps};

impl TermWindow {
    pub fn setup_clipboard(mux_window_id: MuxWindowId) -> anyhow::Result<()> {
        let downloader: Arc<dyn wezterm_term::DownloadHandler> =
            Arc::new(crate::download::Downloader::new());
        let mux = Mux::get().unwrap();

        let mux_window = mux
            .get_window(mux_window_id)
            .ok_or_else(|| anyhow::anyhow!("mux doesn't know about window yet!?"))?;

        for tab in mux_window.iter() {
            for pos in tab.iter_panes() {
                pos.pane.set_download_handler(&downloader);
            }
        }

        Ok(())
    }

    pub fn copy_to_clipboard(&self, clipboard: ClipboardCopyDestination, text: String) {
        let clipboard = match clipboard {
            ClipboardCopyDestination::Clipboard => [Some(Clipboard::Clipboard), None],
            ClipboardCopyDestination::PrimarySelection => [Some(Clipboard::PrimarySelection), None],
            ClipboardCopyDestination::ClipboardAndPrimarySelection => [
                Some(Clipboard::Clipboard),
                Some(Clipboard::PrimarySelection),
            ],
        };
        for &c in &clipboard {
            if let Some(c) = c {
                self.window.as_ref().unwrap().set_clipboard(c, text.clone());
            }
        }
    }

    pub fn paste_from_clipboard(&mut self, pane: &Rc<dyn Pane>, clipboard: ClipboardPasteSource) {
        let pane_id = pane.pane_id();
        log::trace!(
            "paste_from_clipboard in pane {} {:?}",
            pane.pane_id(),
            clipboard
        );
        let window = self.window.as_ref().unwrap().clone();
        let clipboard = match clipboard {
            ClipboardPasteSource::Clipboard => Clipboard::Clipboard,
            ClipboardPasteSource::PrimarySelection => Clipboard::PrimarySelection,
        };
        let future = window.get_clipboard(clipboard);
        promise::spawn::spawn(async move {
            if let Ok(clip) = future.await {
                window.notify(TermWindowNotif::Apply(Box::new(move |myself| {
                    if let Some(pane) = myself.pane_state(pane_id).overlay.clone().or_else(|| {
                        let mux = Mux::get().unwrap();
                        mux.get_pane(pane_id)
                    }) {
                        pane.trickle_paste(clip).ok();
                    }
                })));
            }
        })
        .detach();
        self.maybe_scroll_to_bottom_for_input(&pane);
    }
}
