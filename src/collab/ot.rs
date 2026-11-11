//! Operational transformation for plain text: the same algorithm as ot.js
//! (Google Wave / Etherpad's model). An operation walks the whole document:
//! keep `n` characters, insert a string, or delete `n` characters. Lengths count
//! Unicode scalar values (`char`s), so no edit can split a UTF-8 sequence, and
//! every participant measures them the same way.
//!
//! The host holds the one true copy and orders every edit; each guest runs the
//! small client state machine below (`Client`), so edits typed at the same moment
//! on different PCs always end in the same text everywhere.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comp {
    #[serde(rename = "r")]
    Retain(usize),
    #[serde(rename = "i")]
    Insert(String),
    #[serde(rename = "d")]
    Delete(usize),
}

/// A text operation. `base` is the length of the text it applies to, `target` the
/// length after.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub comps: Vec<Comp>,
}

fn clen(s: &str) -> usize {
    s.chars().count()
}

impl Op {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn retain(&mut self, n: usize) -> &mut Self {
        if n == 0 {
            return self;
        }
        if let Some(Comp::Retain(m)) = self.comps.last_mut() {
            *m += n;
        } else {
            self.comps.push(Comp::Retain(n));
        }
        self
    }

    /// Inserts go before a delete at the same place, so equal edits have one form.
    pub fn insert(&mut self, s: &str) -> &mut Self {
        if s.is_empty() {
            return self;
        }
        match self.comps.as_mut_slice() {
            [.., Comp::Insert(t)] => t.push_str(s),
            [.., Comp::Insert(t), Comp::Delete(_)] => t.push_str(s),
            [.., Comp::Delete(_)] => {
                let d = self.comps.pop().unwrap();
                self.comps.push(Comp::Insert(s.to_string()));
                self.comps.push(d);
            }
            _ => self.comps.push(Comp::Insert(s.to_string())),
        }
        self
    }

    pub fn delete(&mut self, n: usize) -> &mut Self {
        if n == 0 {
            return self;
        }
        if let Some(Comp::Delete(m)) = self.comps.last_mut() {
            *m += n;
        } else {
            self.comps.push(Comp::Delete(n));
        }
        self
    }

    pub fn base_len(&self) -> usize {
        self.comps.iter().map(|c| match c {
            Comp::Retain(n) | Comp::Delete(n) => *n,
            Comp::Insert(_) => 0,
        }).sum()
    }

    pub fn target_len(&self) -> usize {
        self.comps.iter().map(|c| match c {
            Comp::Retain(n) => *n,
            Comp::Insert(s) => clen(s),
            Comp::Delete(_) => 0,
        }).sum()
    }

    /// Characters this operation inserts or deletes. A paste of a megabyte and a
    /// single keystroke are not the same amount of work: the host rebroadcasts
    /// the whole insert to every peer and replays it through the merge machinery.
    pub fn changed_len(&self) -> usize {
        self.comps
            .iter()
            .map(|c| match c {
                Comp::Retain(_) => 0,
                Comp::Insert(s) => clen(s),
                Comp::Delete(n) => *n,
            })
            .sum()
    }

    /// Keeps everything: changes nothing.
    #[cfg(test)]
    pub fn is_noop(&self) -> bool {
        self.comps.iter().all(|c| matches!(c, Comp::Retain(_)))
    }

    pub fn apply(&self, text: &str) -> Result<String, String> {
        let n = clen(text);
        if n != self.base_len() {
            return Err(format!("the edit was made for {} characters, the text has {n}", self.base_len()));
        }
        let mut out = String::with_capacity(text.len());
        let mut it = text.chars();
        for c in &self.comps {
            match c {
                Comp::Retain(k) => out.extend(it.by_ref().take(*k)),
                Comp::Insert(s) => out.push_str(s),
                Comp::Delete(k) => {
                    for _ in 0..*k {
                        it.next();
                    }
                }
            }
        }
        Ok(out)
    }

    /// The single operation that turns `old` into `new`: the common start and end
    /// are kept, the middle replaced. One pass over each string.
    pub fn diff(old: &str, new: &str) -> Op {
        let a: Vec<char> = old.chars().collect();
        let b: Vec<char> = new.chars().collect();
        let pre = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
        let max_suf = a.len().min(b.len()) - pre;
        let suf = a.iter().rev().zip(b.iter().rev()).take(max_suf).take_while(|(x, y)| x == y).count();
        let mut op = Op::new();
        op.retain(pre);
        let ins: String = b[pre..b.len() - suf].iter().collect();
        op.insert(&ins);
        op.delete(a.len() - suf - pre);
        op.retain(suf);
        op
    }

    /// `self` then `other`, as one operation.
    pub fn compose(&self, other: &Op) -> Result<Op, String> {
        if self.target_len() != other.base_len() {
            return Err("compose: lengths differ".into());
        }
        let mut out = Op::new();
        let mut a = self.comps.iter().cloned();
        let mut b = other.comps.iter().cloned();
        let (mut x, mut y) = (a.next(), b.next());
        loop {
            match (x.take(), y.take()) {
                (None, None) => break,
                (Some(Comp::Delete(n)), yy) => {
                    out.delete(n);
                    x = a.next();
                    y = yy;
                }
                (xx, Some(Comp::Insert(s))) => {
                    out.insert(&s);
                    x = xx;
                    y = b.next();
                }
                (None, _) | (_, None) => return Err("compose: operations end at different places".into()),
                (Some(Comp::Retain(n)), Some(Comp::Retain(m))) => {
                    let k = n.min(m);
                    out.retain(k);
                    x = if n > k { Some(Comp::Retain(n - k)) } else { a.next() };
                    y = if m > k { Some(Comp::Retain(m - k)) } else { b.next() };
                }
                (Some(Comp::Insert(s)), Some(Comp::Delete(m))) => {
                    let n = clen(&s);
                    let k = n.min(m);
                    x = if n > k { Some(Comp::Insert(s.chars().skip(k).collect())) } else { a.next() };
                    y = if m > k { Some(Comp::Delete(m - k)) } else { b.next() };
                }
                (Some(Comp::Insert(s)), Some(Comp::Retain(m))) => {
                    let n = clen(&s);
                    let k = n.min(m);
                    out.insert(&s.chars().take(k).collect::<String>());
                    x = if n > k { Some(Comp::Insert(s.chars().skip(k).collect())) } else { a.next() };
                    y = if m > k { Some(Comp::Retain(m - k)) } else { b.next() };
                }
                (Some(Comp::Retain(n)), Some(Comp::Delete(m))) => {
                    let k = n.min(m);
                    out.delete(k);
                    x = if n > k { Some(Comp::Retain(n - k)) } else { a.next() };
                    y = if m > k { Some(Comp::Delete(m - k)) } else { b.next() };
                }
            }
        }
        Ok(out)
    }

    /// For two operations made on the same text, `(a', b')` such that
    /// `b'` after `a` equals `a'` after `b`. When both insert at one place,
    /// `a`'s insert goes first (the host's ordering decides which side is `a`).
    pub fn transform(a: &Op, b: &Op) -> Result<(Op, Op), String> {
        if a.base_len() != b.base_len() {
            return Err("transform: the edits were made on different texts".into());
        }
        let (mut a1, mut b1) = (Op::new(), Op::new());
        let mut ia = a.comps.iter().cloned();
        let mut ib = b.comps.iter().cloned();
        let (mut x, mut y) = (ia.next(), ib.next());
        loop {
            match (x.take(), y.take()) {
                (None, None) => break,
                (Some(Comp::Insert(s)), yy) => {
                    a1.insert(&s);
                    b1.retain(clen(&s));
                    x = ia.next();
                    y = yy;
                }
                (xx, Some(Comp::Insert(s))) => {
                    a1.retain(clen(&s));
                    b1.insert(&s);
                    x = xx;
                    y = ib.next();
                }
                (None, _) | (_, None) => return Err("transform: operations end at different places".into()),
                (Some(Comp::Retain(n)), Some(Comp::Retain(m))) => {
                    let k = n.min(m);
                    a1.retain(k);
                    b1.retain(k);
                    x = if n > k { Some(Comp::Retain(n - k)) } else { ia.next() };
                    y = if m > k { Some(Comp::Retain(m - k)) } else { ib.next() };
                }
                (Some(Comp::Delete(n)), Some(Comp::Delete(m))) => {
                    let k = n.min(m);
                    x = if n > k { Some(Comp::Delete(n - k)) } else { ia.next() };
                    y = if m > k { Some(Comp::Delete(m - k)) } else { ib.next() };
                }
                (Some(Comp::Delete(n)), Some(Comp::Retain(m))) => {
                    let k = n.min(m);
                    a1.delete(k);
                    x = if n > k { Some(Comp::Delete(n - k)) } else { ia.next() };
                    y = if m > k { Some(Comp::Retain(m - k)) } else { ib.next() };
                }
                (Some(Comp::Retain(n)), Some(Comp::Delete(m))) => {
                    let k = n.min(m);
                    b1.delete(k);
                    x = if n > k { Some(Comp::Retain(n - k)) } else { ia.next() };
                    y = if m > k { Some(Comp::Delete(m - k)) } else { ib.next() };
                }
            }
        }
        Ok((a1, b1))
    }

    /// Where a caret at char index `i` ends up after this operation.
    pub fn transform_index(&self, i: usize) -> usize {
        let (mut pos, mut out) = (0usize, i);
        for c in &self.comps {
            if pos > i {
                break;
            }
            match c {
                Comp::Retain(n) => pos += n,
                Comp::Insert(s) => out += clen(s),
                Comp::Delete(n) => {
                    out -= (*n).min(i.saturating_sub(pos));
                    pos += n;
                }
            }
        }
        out
    }

    /// The edits as (char start, chars removed, text inserted) on the original text,
    /// for applying to an editor one span at a time.
    pub fn spans(&self) -> Vec<(usize, usize, String)> {
        let mut out: Vec<(usize, usize, String)> = Vec::new();
        let mut pos = 0;
        for c in &self.comps {
            match c {
                Comp::Retain(n) => pos += n,
                Comp::Insert(s) => match out.last_mut() {
                    Some(last) if last.0 + last.1 == pos => last.2.push_str(s),
                    _ => out.push((pos, 0, s.clone())),
                },
                Comp::Delete(n) => {
                    match out.last_mut() {
                        Some(last) if last.0 + last.1 == pos => last.1 += n,
                        _ => out.push((pos, *n, String::new())),
                    }
                    pos += n;
                }
            }
        }
        out
    }
}

/// A guest's side of one shared file: at most one edit in flight to the host,
/// later local edits gathered into one buffer behind it.
#[derive(Clone, Debug, Default)]
pub enum Client {
    #[default]
    Synced,
    Waiting(Op),
    WaitingBuffer(Op, Op),
}

impl Client {
    /// A local edit. Returns the edit to send now, if nothing is in flight.
    pub fn local(&mut self, op: Op) -> Result<Option<Op>, String> {
        match std::mem::take(self) {
            Client::Synced => {
                *self = Client::Waiting(op.clone());
                Ok(Some(op))
            }
            Client::Waiting(w) => {
                *self = Client::WaitingBuffer(w, op);
                Ok(None)
            }
            Client::WaitingBuffer(w, b) => {
                *self = Client::WaitingBuffer(w, b.compose(&op)?);
                Ok(None)
            }
        }
    }

    /// An edit from the host (someone else's). Returns the form to apply locally.
    pub fn remote(&mut self, op: Op) -> Result<Op, String> {
        match std::mem::take(self) {
            Client::Synced => Ok(op),
            Client::Waiting(w) => {
                let (w1, op1) = Op::transform(&w, &op)?;
                *self = Client::Waiting(w1);
                Ok(op1)
            }
            Client::WaitingBuffer(w, b) => {
                let (w1, op1) = Op::transform(&w, &op)?;
                let (b1, op2) = Op::transform(&b, &op1)?;
                *self = Client::WaitingBuffer(w1, b1);
                Ok(op2)
            }
        }
    }

    /// The host took our in-flight edit. Returns the buffered edit to send next.
    pub fn ack(&mut self) -> Option<Op> {
        match std::mem::take(self) {
            Client::Synced => None,
            Client::Waiting(_) => None,
            Client::WaitingBuffer(_, b) => {
                *self = Client::Waiting(b.clone());
                Some(b)
            }
        }
    }
}

/// The host's copy of one shared file: its text, its revision, and the recent
/// edits a late guest edit is transformed past.
#[derive(Clone, Debug, Default)]
pub struct Server {
    pub text: String,
    pub rev: u64,
    /// Edits `rev - history.len() .. rev`.
    pub history: std::collections::VecDeque<Op>,
}

const HISTORY: usize = 4000;

impl Server {
    pub fn new(text: String) -> Self {
        Self { text, rev: 0, history: Default::default() }
    }

    /// A guest's edit made at revision `base`: transformed past what came since,
    /// applied, recorded. Returns the edit as applied (to send to everyone else).
    pub fn receive(&mut self, base: u64, op: Op) -> Result<Op, String> {
        let oldest = self.rev - self.history.len() as u64;
        if base < oldest || base > self.rev {
            return Err("out of sync".into());
        }
        let mut op = op;
        for past in self.history.iter().skip((base - oldest) as usize) {
            // The host's earlier edit goes first at a shared spot.
            op = Op::transform(past, &op)?.1;
        }
        self.text = op.apply(&self.text)?;
        self.push(op.clone());
        Ok(op)
    }

    /// The host's own edit, made on the current text.
    pub fn local(&mut self, op: Op) -> Result<(), String> {
        self.text = op.apply(&self.text)?;
        self.push(op);
        Ok(())
    }

    fn push(&mut self, op: Op) {
        self.rev += 1;
        self.history.push_back(op);
        if self.history.len() > HISTORY {
            self.history.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Client, Op, Server};

    fn op(base: &str, new: &str) -> Op {
        Op::diff(base, new)
    }

    #[test]
    fn diff_and_apply_round_trip() {
        for (a, b) in [("", "abc"), ("abc", ""), ("hello world", "hello brave world"), ("héllo 😀 x", "héllo 🙂 xy"), ("same", "same")] {
            let o = op(a, b);
            assert_eq!(o.apply(a).unwrap(), b, "{a:?} -> {b:?}");
            assert_eq!(o.base_len(), a.chars().count());
        }
        assert!(op("same", "same").is_noop());
    }

    #[test]
    fn transform_converges() {
        let base = "the quick brown fox";
        let cases = [("the quick brown fox!", "The quick brown fox"), ("the brown fox", "the quick red fox"), ("the quick brown fox", "x"), ("ab the quick brown fox", "cd the quick brown fox")];
        for (x, y) in cases {
            let (a, b) = (op(base, x), op(base, y));
            let (a1, b1) = Op::transform(&a, &b).unwrap();
            let left = b1.apply(&a.apply(base).unwrap()).unwrap();
            let right = a1.apply(&b.apply(base).unwrap()).unwrap();
            assert_eq!(left, right, "{x:?} vs {y:?}");
        }
    }

    #[test]
    fn compose_is_one_then_the_other() {
        let a = op("abc", "abXc");
        let b = op("abXc", "aYbXc!");
        assert_eq!(a.compose(&b).unwrap().apply("abc").unwrap(), "aYbXc!");
    }

    #[test]
    fn caret_moves_with_the_edit() {
        let o = op("hello world", "hello, big world");
        assert_eq!(o.transform_index(0), 0);
        assert_eq!(o.transform_index(6), 11);
        assert_eq!(op("abcdef", "af").transform_index(4), 1);
    }

    /// Two guests type at the same moment; the host orders; everyone ends equal.
    #[test]
    fn host_and_two_guests_end_with_the_same_text() {
        let start = "fn main() {\n}\n".to_string();
        let mut host = Server::new(start.clone());
        let (mut g1, mut g2) = (Client::default(), Client::default());
        let (mut t1, mut t2) = (start.clone(), start.clone());
        // Guest 1 inserts a line; guest 2 renames, both at rev 0.
        let e1 = op(&t1, "fn main() {\n    run();\n}\n");
        t1 = e1.apply(&t1).unwrap();
        let s1 = g1.local(e1).unwrap().unwrap();
        let e2 = op(&t2, "fn start() {\n}\n");
        t2 = e2.apply(&t2).unwrap();
        let s2 = g2.local(e2).unwrap().unwrap();
        // Guest 2 types again before its first edit is acked: buffered.
        let e3 = op(&t2, "fn start() {\n}\n// done\n");
        t2 = e3.apply(&t2).unwrap();
        assert!(g2.local(e3).unwrap().is_none());
        // The host gets guest 1 first, then guest 2 (both made at rev 0).
        let a1 = host.receive(0, s1).unwrap();
        let a2 = host.receive(0, s2).unwrap();
        // Guest 1: ack for its own, then guest 2's edit.
        assert!(g1.ack().is_none());
        t1 = g1.remote(a2).unwrap().apply(&t1).unwrap();
        // Guest 2: guest 1's edit (remote), then ack, then sends its buffer.
        t2 = g2.remote(a1).unwrap().apply(&t2).unwrap();
        let buf = g2.ack().unwrap();
        let a3 = host.receive(2, buf).unwrap();
        assert!(g2.ack().is_none());
        t1 = g1.remote(a3).unwrap().apply(&t1).unwrap();
        assert_eq!(t1, host.text);
        assert_eq!(t2, host.text);
        assert_eq!(host.text, "fn start() {\n    run();\n}\n// done\n");
    }

    #[test]
    fn spans_list_each_change_once() {
        let o = op("abcdef", "aXcdeY");
        let s = o.spans();
        assert_eq!(s, vec![(1, 5, "XcdeY".to_string())]);
        let mut o2 = Op::new();
        o2.retain(1).insert("X").delete(1).retain(2).delete(1).insert("Y").retain(1);
        assert_eq!(o2.apply("abcdef").unwrap(), "aXcdYf");
        assert_eq!(o2.spans(), vec![(1, 1, "X".to_string()), (4, 1, "Y".to_string())]);
    }
}
