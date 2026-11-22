//! The chat list's views: how saved chats are grouped and ordered (by date, most recent, folder, agent), which are working now, and the archive.

use super::Chat;

/// Chats untouched this long go to the archive on their own.
pub const AUTO_ARCHIVE_DAYS: u64 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ChatView {
    /// Today, Yesterday, This week, Earlier (the default).
    #[default]
    Date,
    /// One list, newest first.
    Recent,
    /// By the folder each chat was in.
    Folder,
    /// By the agent (harness) that answered.
    Agent,
    /// Only the chat answering right now.
    Working,
    /// The archive.
    Archived,
}

impl ChatView {
    pub const ALL: [ChatView; 6] = [ChatView::Date, ChatView::Recent, ChatView::Folder, ChatView::Agent, ChatView::Working, ChatView::Archived];

    pub fn label(self) -> &'static str {
        match self {
            ChatView::Date => "By date",
            ChatView::Recent => "Most recent",
            ChatView::Folder => "By folder",
            ChatView::Agent => "By agent",
            ChatView::Working => "Working now",
            ChatView::Archived => "Archived",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            ChatView::Date => "date",
            ChatView::Recent => "recent",
            ChatView::Folder => "folder",
            ChatView::Agent => "agent",
            ChatView::Working => "working",
            ChatView::Archived => "archived",
        }
    }

    pub fn from_id(id: &str) -> ChatView {
        ChatView::ALL.into_iter().find(|v| v.id() == id).unwrap_or_default()
    }
}

/// A group heading (empty for an ungrouped list) and the indexes into `history` it holds, in order.
pub type Groups = Vec<(String, Vec<usize>)>;

/// The chats `view` shows, grouped.
pub fn groups(history: &[Chat], view: ChatView, busy: &[u64], now: u64, today_start: u64) -> Groups {
    let mut idx: Vec<usize> = (0..history.len())
        .filter(|&i| {
            let c = &history[i];
            match view {
                ChatView::Archived => c.archived,
                ChatView::Working => busy.contains(&c.id),
                // A chat working right now always shows, archived or not.
                _ => !c.archived || busy.contains(&c.id),
            }
        })
        .collect();
    // Working first, then pinned, then newest.
    idx.sort_by_key(|&i| (!busy.contains(&history[i].id), !history[i].pinned, std::cmp::Reverse(history[i].updated)));
    let day = 86_400_000;
    let mut out: Groups = Vec::new();
    let mut push = |key: String, i: usize| match out.iter_mut().find(|(k, _)| *k == key) {
        Some((_, v)) => v.push(i),
        None => out.push((key, vec![i])),
    };
    for i in idx {
        let c = &history[i];
        let key = match view {
            ChatView::Recent | ChatView::Working | ChatView::Archived => String::new(),
            _ if busy.contains(&c.id) => "Working now".to_string(),
            _ if c.pinned => "Pinned".to_string(),
            ChatView::Date => {
                let _ = (now, day);
                recency(c.updated, today_start).label().to_string()
            }
            ChatView::Folder => c.folder.as_deref().map(folder_name).unwrap_or_else(|| "No folder".to_string()),
            ChatView::Agent => c.agent.as_deref().map(crate::acp::display_name).unwrap_or_else(|| "Unknown agent".to_string()),
        };
        push(key, i);
    }
    if view == ChatView::Date {
        let rank = |k: &str| match k {
            "Working now" => 0,
            "Pinned" => 1,
            "Today" => 2,
            "This week" => 3,
            "Yesterday" => 4,
            _ => 5,
        };
        out.sort_by_key(|(k, _)| rank(k));
    }
    out
}

/// How recent a chat is, for the date headings and the coloured mark on each row.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Recency {
    Today,
    Yesterday,
    ThisWeek,
    Earlier,
}

impl Recency {
    pub fn label(self) -> &'static str {
        match self {
            Recency::Today => "Today",
            Recency::Yesterday => "Yesterday",
            Recency::ThisWeek => "This week",
            Recency::Earlier => "Earlier",
        }
    }

    /// The heading for this recency, if it is one.
    pub fn of_heading(s: &str) -> Option<Recency> {
        [Recency::Today, Recency::Yesterday, Recency::ThisWeek, Recency::Earlier].into_iter().find(|r| r.label() == s)
    }
}

/// `updated` (Unix ms) against local midnight `today_start`.
pub fn recency(updated: u64, today_start: u64) -> Recency {
    let day = 86_400_000;
    if updated >= today_start {
        Recency::Today
    } else if updated >= today_start.saturating_sub(day) {
        Recency::Yesterday
    } else if updated >= today_start.saturating_sub(6 * day) {
        Recency::ThisWeek
    } else {
        Recency::Earlier
    }
}

/// The compact list: the chats that stay in view (working, open, pinned) and how many are folded behind "Show all".
pub fn compact(groups: Groups, history: &[Chat], busy: &[u64], open: Option<u64>) -> (Groups, usize) {
    let mut hidden = 0;
    let mut out: Groups = Vec::new();
    for (k, idx) in groups {
        let keep: Vec<usize> = idx.iter().copied().filter(|&i| busy.contains(&history[i].id) || history[i].pinned || Some(history[i].id) == open).collect();
        hidden += idx.len() - keep.len();
        if !keep.is_empty() {
            out.push((k, keep));
        }
    }
    (out, hidden)
}

/// The last part of a folder path, for a heading.
pub fn folder_name(path: &str) -> String {
    let t = path.trim_end_matches(['/', '\\']);
    t.rsplit(['/', '\\']).next().filter(|s| !s.is_empty()).unwrap_or(t).to_string()
}

/// Archive what nobody touched for `AUTO_ARCHIVE_DAYS` (except `keep_id`, the open chat, and chats the user took back out of the archive).
pub fn auto_archive(history: &mut [Chat], now: u64, keep_id: Option<u64>) -> bool {
    let limit = AUTO_ARCHIVE_DAYS * 86_400_000;
    let mut moved = false;
    for c in history.iter_mut() {
        if !c.archived && !c.keep && !c.pinned && Some(c.id) != keep_id && now.saturating_sub(c.updated) > limit {
            c.archived = true;
            moved = true;
        }
    }
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chat(id: u64, updated: u64) -> Chat {
        Chat { id, title: format!("c{id}"), renamed: false, updated, messages: Vec::new(), archived: false, keep: false, folder: None, agent: None, account: None, session: None, pinned: false }
    }

    const DAY: u64 = 86_400_000;

    #[test]
    fn date_groups_put_working_first() {
        let today = 100 * DAY;
        let h = vec![chat(1, today + 5), chat(2, today - DAY / 2), chat(3, today - 3 * DAY), chat(4, today - 30 * DAY), chat(5, today - 2 * DAY)];
        let g = groups(&h, ChatView::Date, &[3], today + 10, today);
        let names: Vec<&str> = g.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["Working now", "Today", "This week", "Yesterday", "Earlier"]);
        assert_eq!(g[0].1, vec![2]);
        assert_eq!(g[2].1, vec![4]);
    }

    #[test]
    fn pinned_chats_lead_and_compact_keeps_them() {
        let today = 100 * DAY;
        let mut h = vec![chat(1, today + 5), chat(2, today - 30 * DAY), chat(3, today - 3 * DAY), chat(4, today + 1)];
        h[1].pinned = true;
        let g = groups(&h, ChatView::Date, &[4], today + 10, today);
        let names: Vec<&str> = g.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(names, ["Working now", "Pinned", "Today", "This week"]);
        // Compact: working, pinned and the open chat stay; two are folded away.
        let (c, hidden) = compact(g, &h, &[4], Some(3));
        let kept: Vec<usize> = c.iter().flat_map(|(_, v)| v.clone()).collect();
        assert_eq!(kept, vec![3, 1, 2]);
        assert_eq!(hidden, 1);
        // A pinned chat is never archived on its own.
        assert!(!auto_archive(&mut h[1..2], today, None));
        assert_eq!(recency(today - DAY / 2, today), Recency::Yesterday);
        assert_eq!(Recency::of_heading("This week"), Some(Recency::ThisWeek));
    }

    #[test]
    fn archive_is_its_own_view() {
        let mut h = vec![chat(1, 10 * DAY), chat(2, 1 * DAY), chat(3, 2 * DAY)];
        h[2].keep = true;
        assert!(auto_archive(&mut h, 10 * DAY + 1, None));
        assert!(!h[0].archived && h[1].archived && !h[2].archived);
        let recent = groups(&h, ChatView::Recent, &[], 0, 0);
        assert_eq!(recent, vec![(String::new(), vec![0, 2])]);
        assert_eq!(groups(&h, ChatView::Archived, &[], 0, 0), vec![(String::new(), vec![1])]);
        // Nothing left to move.
        assert!(!auto_archive(&mut h, 10 * DAY + 1, None));
    }

    #[test]
    fn folders_group_by_their_last_part() {
        let mut h = vec![chat(1, 3), chat(2, 2), chat(3, 1)];
        h[0].folder = Some(r"C:\work\api".into());
        h[1].folder = Some("/home/me/web/".into());
        h[2].folder = Some(r"C:\work\api".into());
        let g = groups(&h, ChatView::Folder, &[], 0, 0);
        assert_eq!(g, vec![("api".to_string(), vec![0, 2]), ("web".to_string(), vec![1])]);
    }

    #[test]
    fn several_chats_work_at_once() {
        let today = 100 * DAY;
        let mut h = vec![chat(1, today + 3), chat(2, today + 2), chat(3, today - 20 * DAY), chat(4, today + 1)];
        // An archived chat still shows while its agent answers.
        h[2].archived = true;
        let g = groups(&h, ChatView::Date, &[2, 3], today + 10, today);
        assert_eq!(g[0], ("Working now".to_string(), vec![1, 2]));
        assert_eq!(g[1], ("Today".to_string(), vec![0, 3]));
        assert_eq!(groups(&h, ChatView::Working, &[2, 3], 0, 0), vec![(String::new(), vec![1, 2])]);
        assert_eq!(groups(&h, ChatView::Working, &[], 0, 0), Vec::new());
    }
}
