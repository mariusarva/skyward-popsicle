//! The decrypt page: where the data partition stands, and the control a
//! user's screen lock is answered with.

use super::*;

/// What work that needs the data partition is told while it is being
/// opened.
pub const DATA_OPENING: Text = ["正在打开 data 分区，请稍后再试", "正在開啟 data 分割區，請稍後再試", "Opening the data partition. Try again shortly"];

/// What work on the data partition is told once it has been erased whole.
pub const DATA_ERASED: Text = ["data 分区已清除", "data 分割區已清除", "The data partition was wiped"];

/// What work on internal storage is told before the data partition is
/// decrypted.
pub const DECRYPT_FIRST: Text = ["请先完成「解密」", "請先完成「解密」", "Decrypt first"];

/// What the page says when the screen lock kept from before is taking too
/// long to be counted: it asks for the lock rather than wait on the secure
/// world any longer.
pub const REMEMBER_SLOW: Text = [
    "保存的密码验证太慢，请手动输入",
    "已儲存的密碼驗證太慢，請手動輸入",
    "The kept screen lock is slow to check. Type it instead",
];

/// What the page says when nothing is opening the partition any more: the
/// request failed where it was made, and the page offers to try it again.
pub const OPEN_LOST: Text = [
    "打开 data 分区停了，请重试",
    "開啟 data 分割區停了，請重試",
    "Opening the data partition stopped. Try again",
];

/// Which control answers a user's screen lock, by the kind of lock it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keys {
    Pattern,
    Digits,
    Letters,
}

/// Where the data partition stands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Data {
    /// Not opened; the decrypt page opens it as it shows.
    #[default]
    Closed,
    /// Being opened as far as it opens with nothing a user knows, which is
    /// also when each user's screen lock is read.
    Opening,
    /// Open, with this user's own files waiting on their screen lock,
    /// answered with `keys`; `checking` while what they entered is checked.
    Locked { user: u32, keys: Keys, checking: bool },
    /// Open, every user's own files with it.
    Open,
    /// Not opened, and why.
    Failed(String),
    /// Erased whole: nothing opens until the system makes it afresh, as it
    /// next starts.
    Erased,
}

impl Data {
    /// Whether the owner's internal storage can be read: the partition is
    /// open, and so are the owner's own files, which open first.
    pub fn storage_open(&self) -> bool {
        match self {
            Data::Open => true,
            // The owner is user 0.
            Data::Locked { user, .. } => *user != 0,
            _ => false,
        }
    }
}

impl App {
    /// The data partition is open: every user's own files with it, or
    /// those of `waiting`, the user and the control their screen lock is
    /// answered with, still closed.
    /// The picture the phone shows for its owner has been read, and the name
    /// it gives itself: the home page shows them over the phone's own row.
    pub fn set_avatar(&mut self, picture: crate::avatar::Avatar) {
        self.state.avatar = Some(picture);
        self.refresh();
    }

    pub fn set_device_name(&mut self, name: String) {
        self.state.device_name = Some(name);
        self.refresh();
    }

    pub fn data_waiting(&mut self, waiting: Option<(u32, Keys)>) {
        match waiting {
            Some((user, keys)) => {
                self.data_back = None;
                self.data_finish = None;
                self.data_finish_at = None;
                self.state.data = Data::Locked { user, keys, checking: false };
            }
            None => {
                // The bar runs its last length before the page says the
                // partition is open: a bar frozen part way while the page
                // already says it is done reads as two different jobs.
                if self.state.data_progress < 0.999 {
                    self.state.data_reported = 1.0;
                    self.data_finish_at = None;
                    self.data_finish_from = self.state.data_progress;
                    self.data_finish = Some(DataAfter::Open);
                } else {
                    self.state.data = Data::Open;
                }
            }
        }
        self.state.code.clear();
        self.refresh();
    }

    /// The partition is open and the owner's own files are being opened
    /// behind the lock kept from before: the page goes on saying it is at
    /// the partition rather than asking for what it already has.
    pub fn data_still_opening(&mut self) {
        if let Data::Locked { user, keys, .. } = self.state.data {
            self.data_back = Some((user, keys));
            self.state.data = Data::Opening;
            self.refresh();
        }
    }

    /// The data partition opened with every user's files that the recovery can
    /// open: the decrypt page is done with, and the pages behind it are what to
    /// show. Called once what was entered opened the owner's own files, so that
    /// the page does not stay in front of a partition that is already open.
    pub fn decrypted(&mut self) {
        // The partition is open, so the decrypt page has done its work: the
        // menu is what to show, whether or not it was this page that opened it
        // — a start that kept the lock opens it with nobody looking at one,
        // and a lock tried again after the page stopped waiting for it opens
        // it just as well (2026-11-19).
        // The decrypt page is not the one shown — a start that kept the lock
        // opens the partition with nobody looking at one, and the user may
        // have gone elsewhere while it did: the pages behind it read as open.
        if self.screen() != Screen::Decrypt {
            self.data_finish = None;
            self.data_finish_at = None;
            self.data_back = None;
            self.state.data = Data::Open;
            self.refresh();
            return;
        }
        // The bar runs its last length before the page leaves: gone at once,
        // it would never be seen full, and the page would jump under the eye.
        // The leaving itself is left to the tick, which is where the page's
        // frame is at hand: taken here, straight after a tick that let it go
        // for the ring to turn, the page goes with nothing to slide away and
        // the turn panics, leaving the page in front of the menu for good
        // (2026-11-19).
        self.state.data_reported = 1.0;
        if self.data_finish != Some(DataAfter::Home) {
            self.data_finish_at = None;
            self.data_finish_from = self.state.data_progress;
        }
        self.data_finish = Some(DataAfter::Home);
        self.refresh();
    }

    /// Leave the decrypt page for the menu: the partition is open, and the
    /// bar on the page has run its length.
    pub(super) fn leave_decrypt(&mut self) {
        self.data_finish = None;
        self.data_finish_at = None;
        self.data_back = None;
        self.state.data = Data::Open;
        self.leave(true);
        while self.stack.len() > 1 {
            self.stack.pop();
        }
        if let Some(top) = self.stack.last_mut() {
            *top = (Screen::Home, 0.0, Focus::Unset);
        }
        self.settle_page();
        self.refresh();
    }

    /// How far opening the partition has come, in thousandths, as the work
    /// itself reports it: the bar on the page steps on toward it, and never
    /// back.
    pub fn set_data_progress(&mut self, permille: u16) {
        let far = (permille.min(1000) as f32 / 1000.0).clamp(0.0, 0.999);
        if far > self.state.data_reported {
            self.state.data_reported = far;
        }
    }

    /// Whether the partition is open and the page is only running the bar's
    /// last length before it leaves or says so. Nothing is working on the
    /// partition then, and that is not a page waiting on nothing.
    pub fn data_finishing(&self) -> bool {
        self.data_finish.is_some()
    }

    /// The data partition did not open, and why.
    pub fn data_failed(&mut self, why: String) {
        self.state.data = Data::Failed(why);
        self.data_finish = None;
        self.data_finish_at = None;
        self.data_back = None;
        self.refresh();
    }

    /// Nothing is working on the partition any more while the page still says
    /// it is: where the attempt was made with the lock kept from before, the
    /// page asks for that lock again; where there was no such attempt, it says
    /// the opening stopped and offers to try it again. A page left saying it
    /// is at a partition nothing is opening is a page that never gets in
    /// (2026-11-19).
    pub fn data_gave_up(&mut self, why: String, now: u64) {
        if let Some((user, keys)) = self.data_back.take() {
            self.state.data = Data::Locked { user, keys, checking: false };
            self.data_finish = None;
            self.data_finish_at = None;
            self.refresh();
            self.report(&why, now);
            return;
        }
        if matches!(self.state.data, Data::Closed | Data::Opening) {
            self.data_failed(why);
        }
    }

    /// What the user entered did not open their own files: the page asks
    /// again and says why.
    pub fn unlock_failed(&mut self, why: String, now: u64) {
        if let Data::Locked { checking, .. } = &mut self.state.data {
            *checking = false;
        } else if let Some((user, keys)) = self.data_back.take() {
            // What was tried was the lock kept from before, and the page had
            // gone on saying it was at the partition: it asks for the lock
            // again rather than waiting on a job that has ended.
            self.state.data = Data::Locked { user, keys, checking: false };
            self.data_finish = None;
            self.data_finish_at = None;
        }
        self.refresh();
        self.report(&why, now);
    }

    /// Open the data partition, the decrypt page saying so meanwhile.
    pub(super) fn open_data(&mut self) -> Vec<Effect> {
        self.state.data = Data::Opening;
        self.state.data_reported = 0.0;
        self.state.data_progress = 0.0;
        self.data_moved = 0;
        self.data_started = None;
        self.data_finish = None;
        self.data_finish_at = None;
        self.data_finish_from = 0.0;
        self.data_back = None;
        self.refresh();
        vec![Effect::OpenData]
    }

    /// Send what was entered at the screen lock of the user whose own
    /// files wait. It is taken as it is sent, so the dots clear while it is
    /// checked, and nothing more is entered until it has been.
    pub(super) fn unlock(&mut self, code: String) -> Vec<Effect> {
        // Only a screen lock waiting takes what is entered; a page that has
        // moved on since the key was drawn drops it rather than ending the
        // recovery over it.
        let Data::Locked { user, keys, .. } = self.state.data else { return Vec::new() };
        self.state.data = Data::Locked { user, keys, checking: true };
        self.refresh();
        vec![Effect::Unlock { user, code }]
    }

    /// Add the dot under the finger to the pattern being drawn.
    pub(super) fn trace(&mut self, t: &Theme, x: f32, y: f32) {
        let Some((cx, cy, span)) = self.frame.as_ref().and_then(|fr| fr.pattern) else { return };
        let reach = t.m.dp(widgets::PATTERN_REACH_DP);
        let dots = widgets::pattern_dots(cx, cy, span);
        if let Some(i) = dots.iter().position(|&(dx, dy)| (dx - x).hypot(dy - y) < reach) {
            self.join(t, i);
        }
    }

    /// Join dot `i` to the pattern being drawn, after the dot it passes
    /// straight over on its way from the last one, as the system's pattern
    /// lock joins that one too. A dot joined once is not joined again.
    pub(super) fn join(&mut self, t: &Theme, i: usize) {
        let drawn = &mut self.state.pattern;
        if drawn.contains(&i) {
            return;
        }
        if let Some(&last) = drawn.last() {
            let (row, col) = ((last / 3) as i32, (last % 3) as i32);
            let (rows, cols) = ((i / 3) as i32 - row, (i % 3) as i32 - col);
            // Two rows or columns away, and not a knight's move.
            let row = if rows.abs() == 2 && cols.abs() != 1 { row + rows.signum() } else { row };
            let col = if cols.abs() == 2 && rows.abs() != 1 { col + cols.signum() } else { col };
            let over = (row * 3 + col) as usize;
            if !drawn.contains(&over) {
                drawn.push(over);
            }
        }
        drawn.push(i);
        self.repaint_pattern(t);
    }

    pub(super) fn repaint_pattern(&mut self, t: &Theme) {
        let Some(fr) = self.frame.as_mut() else { return };
        let Some((cx, cy, span)) = fr.pattern else { return };
        let r = span / 2.0 + t.m.dp(16.0);
        fr.canvas.fill_rect(cx - r, cy - r, r * 2.0, r * 2.0, t.pal.bg);
        widgets::pattern_lock(&mut fr.canvas, t, cx, cy, span, &self.state.pattern);
        self.layer = None;
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::testing::*;

    /// Whether the page shows a key of the keypad or keyboard to tap.
    fn keys_shown(app: &mut App, f: &Fonts, i: &DeviceInfo) -> bool {
        app.frame(f, i).hits.iter().any(|h| matches!(h.action, Action::Type(_)))
    }

    #[test]
    fn the_decrypt_page_opens_the_partition_and_then_asks_with_the_users_own_control() {
        let (f, i) = (fonts(), info());
        // The decrypt page is on the advanced page now.
        let mut app = App::new(Screen::Advanced, W, H, PPD);
        assert_eq!(tap(&mut app, &f, &i, Action::Open(Screen::Decrypt)), vec![Effect::OpenData], "no swipe first");
        assert_eq!(app.state.data, Data::Opening);
        assert!(!keys_shown(&mut app, &f, &i) && app.frame(&f, &i).pattern.is_none(), "nothing to answer yet");
        app.data_waiting(Some((0, Keys::Pattern)));
        assert!(app.frame(&f, &i).pattern.is_some(), "the owner set a pattern, so the grid shows");
        app.data_waiting(Some((0, Keys::Digits)));
        assert!(keys_shown(&mut app, &f, &i), "for a code, the keypad");
        app.key(Key::Back, 0);
        assert!(tap(&mut app, &f, &i, Action::Open(Screen::Decrypt)).is_empty(), "open already, it is not opened again");
        let mut first = App::new(Screen::Decrypt, W, H, PPD);
        assert_eq!(first.arrive(), vec![Effect::OpenData], "a recovery that starts on the page opens it as well");
    }

    #[test]
    fn leaving_the_decrypt_page_goes_back_to_where_it_was_opened_from() {
        let (f, i) = (fonts(), info());
        let mut app = opened(Screen::Decrypt, &f, &i);
        tap(&mut app, &f, &i, Action::Skip);
        assert_eq!((app.screen(), app.stack.len()), (Screen::Home, 1), "back on the home page, with no other behind it");
        let mut first = App::new(Screen::Decrypt, W, H, PPD);
        first.arrive();
        tap(&mut first, &f, &i, Action::Skip);
        assert_eq!((first.screen(), first.stack.len()), (Screen::Home, 1), "a recovery that started on the page goes on to the home page");
    }

    #[test]
    fn the_code_typed_on_the_keypad_is_sent_once_and_checked_before_more_is_taken() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Decrypt, W, H, PPD);
        app.data_waiting(Some((0, Keys::Digits)));
        for ch in "1234".chars() {
            tap(&mut app, &f, &i, Action::Type(Cap::Char(ch)));
        }
        assert_eq!(app.state.code, "1234");
        tap(&mut app, &f, &i, Action::Type(Cap::Backspace));
        assert_eq!(app.state.code, "123");
        let fx = tap(&mut app, &f, &i, Action::Type(Cap::Enter));
        assert_eq!(fx, vec![Effect::Unlock { user: 0, code: "123".into() }]);
        assert_eq!(app.state.code, "", "what was typed is gone once it is sent");
        assert_eq!(app.state.data, Data::Locked { user: 0, keys: Keys::Digits, checking: true });
        assert!(!keys_shown(&mut app, &f, &i), "nothing more is taken while it is checked");
        app.unlock_failed("the secure world does not recognise that credential".into(), 0);
        assert!(keys_shown(&mut app, &f, &i), "turned down, it is asked for again");
        assert_eq!(app.deadline(0), Some(TOAST_MS), "with why");
        app.data_waiting(None);
        assert_eq!(app.state.data, Data::Open);
    }

    #[test]
    fn nothing_typed_is_not_sent() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Decrypt, W, H, PPD);
        app.data_waiting(Some((0, Keys::Digits)));
        assert!(tap(&mut app, &f, &i, Action::Type(Cap::Enter)).is_empty());
        assert_eq!(app.deadline(0), Some(TOAST_MS), "the page says to type one first");
    }

    #[test]
    fn a_pattern_is_sent_as_the_numbers_of_the_dots_it_joins() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Decrypt, W, H, PPD);
        app.data_waiting(Some((0, Keys::Pattern)));
        let (cx, cy, span) = app.frame(&f, &i).pattern.expect("the grid is on the page");
        let dots = widgets::pattern_dots(cx, cy, span);
        let scroll = app.top().1;
        app.touch(Touch::Down(dots[0].0, dots[0].1 - scroll), &f, 0);
        for &which in &[1usize, 4, 7] {
            app.touch(Touch::Move(dots[which].0, dots[which].1 - scroll), &f, 0);
        }
        let fx = app.touch(Touch::Up, &f, 0);
        assert_eq!(fx, vec![Effect::Unlock { user: 0, code: "1258".into() }], "the dots counted from one across the rows");
        assert!(app.state.pattern.is_empty(), "the grid is cleared once it is sent");
        assert!(app.frame(&f, &i).hits.iter().all(|h| h.action != Action::Pattern), "nothing more is drawn while it is checked");
    }

    #[test]
    fn a_pattern_too_short_to_be_one_is_not_sent() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Decrypt, W, H, PPD);
        app.data_waiting(Some((0, Keys::Pattern)));
        let (cx, cy, span) = app.frame(&f, &i).pattern.expect("the grid is on the page");
        let dots = widgets::pattern_dots(cx, cy, span);
        let scroll = app.top().1;
        app.touch(Touch::Down(dots[0].0, dots[0].1 - scroll), &f, 0);
        app.touch(Touch::Move(dots[1].0, dots[1].1 - scroll), &f, 0);
        let fx = app.touch(Touch::Up, &f, 0);
        assert!(fx.is_empty());
        assert_eq!(app.deadline(0), Some(TOAST_MS), "the page says how many dots it takes");
    }

    #[test]
    fn a_partition_that_did_not_open_says_why_and_is_tried_again() {
        let (f, i) = (fonts(), info());
        let mut app = App::new(Screen::Decrypt, W, H, PPD);
        app.arrive();
        app.data_failed("KeyMint is not running".into());
        assert_eq!(tap(&mut app, &f, &i, Action::OpenData), vec![Effect::OpenData]);
        assert_eq!(app.state.data, Data::Opening);
    }
}
