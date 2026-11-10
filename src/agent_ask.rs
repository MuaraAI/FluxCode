//! Questions from the agent (ACP `elicitation/create`): the agent asks, the user picks
//! A, B, C... or types an answer of their own, and the answer goes back as the form's
//! content. This is how Claude Agent's "ask the user" and OpenCode's question tool
//! reach an ACP client. FluxCode advertises the capability in `initialize`.
//!
//! A form is a flat JSON Schema: each property is one question.
//! - string with `enum` / `oneOf`: one choice, lettered, plus "Type my own answer"
//! - array of such strings: several choices (checkboxes)
//! - boolean: Yes / No
//! - number, integer, plain string: a text box
//! A URL request ("open this page to sign in") shows the address and an Open button.

use crate::i18n::tr;
use crate::theme::{self, c};
use crate::workbench::Workbench;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    text::TextView,
    v_flex, Icon, Sizable,
};
use gpui_kit::{prelude::FluentBuilder, *};
use serde_json::{json, Map, Value};

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// One of the options (`multi`: any number of them).
    Choice { options: Vec<Opt>, multi: bool },
    Bool,
    /// Typed: `number` / `integer` keep their JSON type in the answer.
    Text { json_type: &'static str },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    pub value: String,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub title: String,
    pub description: String,
    pub required: bool,
    pub kind: Kind,
}

/// One question form waiting for the user.
pub struct Ask {
    pub rpc_id: Value,
    pub message: String,
    pub fields: Vec<Field>,
    /// Per field: the chosen option indices (`OTHER` = the typed answer).
    pub picked: Vec<Vec<usize>>,
    /// A URL request: the page to open.
    pub url: Option<String>,
}

/// The "Type my own answer" pick.
pub const OTHER: usize = usize::MAX;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn options_of(p: &Value) -> Option<Vec<Opt>> {
    if let Some(e) = p.get("enum").and_then(Value::as_array) {
        let names = p.get("enumNames").and_then(Value::as_array);
        return Some(
            e.iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    let value = v.as_str()?.to_string();
                    let title = names.and_then(|n| n.get(i)).and_then(Value::as_str).map(String::from).unwrap_or_else(|| value.clone());
                    Some(Opt { value, title, description: String::new() })
                })
                .collect(),
        );
    }
    let list = p.get("oneOf").or_else(|| p.get("anyOf")).and_then(Value::as_array)?;
    Some(
        list.iter()
            .filter_map(|o| {
                let value = o.get("const").and_then(Value::as_str)?.to_string();
                let title = o.get("title").and_then(Value::as_str).map(String::from).unwrap_or_else(|| value.clone());
                Some(Opt { value, title, description: s(o, "description") })
            })
            .collect(),
    )
}

/// The questions of a `requestedSchema`.
pub fn parse_schema(schema: &Value) -> Vec<Field> {
    let required: Vec<String> = schema.get("required").and_then(Value::as_array).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();
    let Some(props) = schema.get("properties").and_then(Value::as_object) else { return Vec::new() };
    props
        .iter()
        .map(|(key, p)| {
            let ty = p.get("type").and_then(Value::as_str).unwrap_or("string");
            let kind = match ty {
                "boolean" => Kind::Bool,
                "array" => match p.get("items").and_then(options_of) {
                    Some(options) => Kind::Choice { options, multi: true },
                    None => Kind::Text { json_type: "string" },
                },
                "number" => Kind::Text { json_type: "number" },
                "integer" => Kind::Text { json_type: "integer" },
                _ => match options_of(p) {
                    Some(options) if !options.is_empty() => Kind::Choice { options, multi: false },
                    _ => Kind::Text { json_type: "string" },
                },
            };
            let title = p.get("title").and_then(Value::as_str).map(String::from).unwrap_or_else(|| key.clone());
            Field { key: key.clone(), title, description: s(p, "description"), required: required.contains(key), kind }
        })
        .collect()
}

/// A letter for option `i`: A..Z, then AA.. (forms rarely go past D).
pub fn letter(i: usize) -> String {
    if i < 26 { ((b'A' + i as u8) as char).to_string() } else { format!("{}{}", letter(i / 26 - 1), letter(i % 26)) }
}

/// The form's `content`, or the first question still unanswered (its title).
pub fn answer(fields: &[Field], picked: &[Vec<usize>], typed: &[String]) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (i, f) in fields.iter().enumerate() {
        let sel = picked.get(i).map(Vec::as_slice).unwrap_or(&[]);
        let text = typed.get(i).map(|t| t.trim().to_string()).unwrap_or_default();
        let value = match &f.kind {
            Kind::Bool => sel.first().map(|&k| Value::Bool(k == 0)),
            Kind::Choice { options, multi: false } => match sel.first() {
                Some(&OTHER) if !text.is_empty() => Some(Value::String(text)),
                Some(&k) if k != OTHER => options.get(k).map(|o| Value::String(o.value.clone())),
                _ => None,
            },
            Kind::Choice { options, multi: true } => {
                let mut v: Vec<Value> = sel.iter().filter_map(|&k| options.get(k).map(|o| Value::String(o.value.clone()))).collect();
                if sel.contains(&OTHER) && !text.is_empty() {
                    v.push(Value::String(text));
                }
                (!v.is_empty()).then_some(Value::Array(v))
            }
            Kind::Text { json_type } if !text.is_empty() => match *json_type {
                "integer" => Some(text.parse::<i64>().map(Value::from).map_err(|_| f.title.clone())?),
                "number" => Some(text.parse::<f64>().map(|n| json!(n)).map_err(|_| f.title.clone())?),
                _ => Some(Value::String(text)),
            },
            Kind::Text { .. } => None,
        };
        match value {
            Some(v) => {
                out.insert(f.key.clone(), v);
            }
            None if f.required => return Err(f.title.clone()),
            None => {}
        }
    }
    Ok(out)
}

impl Workbench {
    /// An `elicitation/create` request: show its questions (or its URL) above the composer.
    pub(crate) fn agent_on_elicitation(&mut self, id: Value, params: &Value, cx: &mut Context<Self>) {
        let message = s(params, "message");
        let url = (params.get("mode").and_then(Value::as_str) == Some("url")).then(|| s(params, "url")).filter(|u| !u.is_empty());
        let fields = params.get("requestedSchema").map(parse_schema).unwrap_or_default();
        if url.is_none() && fields.is_empty() {
            // Nothing FluxCode can show: say so instead of leaving the agent waiting.
            self.agent_write(json!({ "jsonrpc": "2.0", "id": id, "result": { "action": "decline" } }));
            return;
        }
        let picked = vec![Vec::new(); fields.len()];
        for i in 0..fields.len().min(self.agent.ask_inputs.len()) {
            self.agent.ask_clear.push(i);
        }
        self.agent.ask = Some(Ask { rpc_id: id, message, fields, picked, url });
        cx.notify();
    }

    /// Send the answers (`Some(true)` accept, `Some(false)` decline, `None` cancel).
    pub(crate) fn agent_answer_ask(&mut self, accept: Option<bool>, cx: &mut Context<Self>) {
        let Some(ask) = self.agent.ask.as_ref() else { return };
        let result = match accept {
            Some(true) if ask.url.is_some() => json!({ "action": "accept" }),
            Some(true) => {
                let typed: Vec<String> = self.agent.ask_inputs.iter().map(|i| i.read(cx).value().to_string()).collect();
                match answer(&ask.fields, &ask.picked, &typed) {
                    Ok(content) => json!({ "action": "accept", "content": content }),
                    Err(missing) => {
                        self.agent.ask_missing = Some(missing);
                        cx.notify();
                        return;
                    }
                }
            }
            Some(false) => json!({ "action": "decline" }),
            None => json!({ "action": "cancel" }),
        };
        let Some(ask) = self.agent.ask.take() else { return };
        self.agent.ask_missing = None;
        self.agent_write(json!({ "jsonrpc": "2.0", "id": ask.rpc_id, "result": result }));
        cx.notify();
    }

    fn agent_pick(&mut self, field: usize, opt: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ask) = self.agent.ask.as_mut() else { return };
        let multi = matches!(ask.fields.get(field).map(|f| &f.kind), Some(Kind::Choice { multi: true, .. }));
        let sel = &mut ask.picked[field];
        if multi {
            if let Some(p) = sel.iter().position(|&k| k == opt) {
                sel.remove(p);
            } else {
                sel.push(opt);
            }
        } else {
            *sel = vec![opt];
        }
        self.agent.ask_missing = None;
        if opt == OTHER {
            if let Some(input) = self.agent.ask_inputs.get(field) {
                let h = input.focus_handle(cx);
                h.focus(window, cx);
            }
        }
        // A single question with a single choice answers on click, as in Claude Code.
        let one_click = ask.fields.len() == 1 && !multi && opt != OTHER;
        cx.notify();
        if one_click {
            self.agent_answer_ask(Some(true), cx);
        }
    }

    pub(crate) fn render_agent_ask(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let ask = self.agent.ask.as_ref()?;
        let row = |id: SharedString, key: String, title: String, desc: String, on: bool, multi: bool| {
            h_flex()
                .id(id)
                .w_full()
                .px_2()
                .py_1p5()
                .gap_2()
                .items_start()
                .rounded(px(6.))
                .border_1()
                .border_color(c(if on { theme::ACCENT } else { theme::BORDER }))
                .when(on, |d| d.bg(c(theme::SELECT)))
                .cursor_pointer()
                .hover(|s| s.border_color(c(theme::BORDER_STRONG)))
                .child(
                    div()
                        .flex_none()
                        .mt(px(1.))
                        .size(theme::u(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(if multi { 4. } else { 10. }))
                        .border_1()
                        .border_color(c(if on { theme::ACCENT } else { theme::BORDER_STRONG }))
                        .when(on, |d| d.bg(c(theme::ACCENT)))
                        .text_size(theme::u(11.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(c(if on { theme::ON_ACCENT } else { theme::FG_MUTED }))
                        .child(key),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(title))
                        .when(!desc.is_empty(), |v| v.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(desc))),
                )
        };
        let mut body = v_flex().gap_3();
        if let Some(url) = &ask.url {
            body = body.child(
                h_flex()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(c(theme::BORDER))
                    .child(Icon::new(gpui_kit::assets::IconName::ExternalLink).size(theme::u(14.)).text_color(c(theme::FG_MUTED)))
                    .child(div().flex_1().min_w_0().truncate().text_xs().text_color(c(theme::FG)).child(url.clone())),
            );
        }
        for (fi, f) in ask.fields.iter().enumerate() {
            let sel = &ask.picked[fi];
            let mut q = v_flex().gap_1p5();
            if ask.fields.len() > 1 || f.title != f.key {
                q = q.child(
                    h_flex()
                        .gap_1()
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(c(theme::FG_STRONG)).child(f.title.clone()))
                        .when(f.required && ask.fields.len() > 1, |h| h.child(div().text_xs().text_color(c(theme::FG_DIM)).child("*"))),
                );
            }
            if !f.description.is_empty() {
                q = q.child(div().text_xs().text_color(c(theme::FG_MUTED)).child(f.description.clone()));
            }
            match &f.kind {
                Kind::Choice { options, multi } => {
                    for (oi, o) in options.iter().enumerate() {
                        let on = sel.contains(&oi);
                        q = q.child(row(SharedString::from(format!("ask-{fi}-{oi}")), letter(oi), o.title.clone(), o.description.clone(), on, *multi).on_click(cx.listener(move |this, _, w, cx| this.agent_pick(fi, oi, w, cx))));
                    }
                    let on = sel.contains(&OTHER);
                    q = q.child(row(SharedString::from(format!("ask-{fi}-other")), letter(options.len()), tr("Type my own answer").into(), String::new(), on, *multi).on_click(cx.listener(move |this, _, w, cx| this.agent_pick(fi, OTHER, w, cx))));
                    if on {
                        if let Some(input) = self.agent.ask_inputs.get(fi) {
                            q = q.child(div().pl(theme::u(28.)).child(Input::new(input).small()));
                        }
                    }
                }
                Kind::Bool => {
                    for (oi, label) in [(0usize, tr("Yes")), (1, tr("No"))] {
                        let on = sel.contains(&oi);
                        q = q.child(row(SharedString::from(format!("ask-{fi}-{oi}")), letter(oi), label.into(), String::new(), on, false).on_click(cx.listener(move |this, _, w, cx| this.agent_pick(fi, oi, w, cx))));
                    }
                }
                Kind::Text { .. } => {
                    if let Some(input) = self.agent.ask_inputs.get(fi) {
                        q = q.child(Input::new(input).small());
                    }
                }
            }
            body = body.child(q);
        }
        let single_click = ask.fields.len() == 1 && matches!(ask.fields[0].kind, Kind::Choice { multi: false, .. } | Kind::Bool) && !ask.picked[0].contains(&OTHER);
        let is_url = ask.url.is_some();
        let url = ask.url.clone();
        let footer = h_flex()
            .gap_2()
            .items_center()
            .when_some(self.agent.ask_missing.clone(), |h, m| h.child(div().flex_1().min_w_0().text_xs().text_color(c(theme::WARN)).child(crate::trf!("Answer “{q}” first."; q => m))))
            .when(self.agent.ask_missing.is_none(), |h| {
                h.child(div().flex_1().min_w_0().text_xs().text_color(c(theme::FG_DIM)).child(if single_click { tr("Pick one, or type your own.") } else { tr("Your answer goes back to the agent.") }))
            })
            .child(Button::new("ask-skip").label(tr("Skip")).xsmall().ghost().on_click(cx.listener(|this, _, _, cx| this.agent_answer_ask(Some(false), cx))))
            .when(!single_click || is_url, |h| {
                h.child(Button::new("ask-send").label(if is_url { tr("Open the page") } else { tr("Send answer") }).xsmall().primary().on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(u) = &url {
                        cx.open_url(u);
                    }
                    this.agent_answer_ask(Some(true), cx)
                })))
            });
        Some(
            v_flex()
                .id("ag-ask")
                .flex_none()
                .mx_3()
                .mb_2()
                .p_3()
                .gap_3()
                .rounded(px(8.))
                .border_1()
                .border_color(c(theme::ACCENT))
                .bg(c(theme::BG_ELEVATED))
                .max_h(theme::u(460.))
                .child(
                    h_flex()
                        .gap_2()
                        .items_start()
                        .child(Icon::new(gpui_kit::assets::IconName::CircleQuestionMark).size(theme::u(16.)).text_color(c(theme::ACCENT_TEXT)))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(div().text_xs().text_color(c(theme::FG_MUTED)).child(crate::trf!("{name} asks"; name => crate::agent::agent_name())))
                                .child(div().text_sm().text_color(c(theme::FG_STRONG)).child(TextView::markdown("ag-ask-msg", ask.message.clone()))),
                        ),
                )
                // Only the questions scroll; the question and the buttons stay in view.
                .child(div().id("ag-ask-body").flex_1().min_h_0().overflow_y_scroll().child(body))
                .child(footer)
                .into_any_element(),
        )
    }

    /// Empty the text boxes for a new form (render has the Window).
    pub(crate) fn apply_ask_clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for i in std::mem::take(&mut self.agent.ask_clear) {
            if let Some(input) = self.agent.ask_inputs.get(i) {
                input.update(cx, |s, cx| s.set_value("", window, cx));
            }
        }
    }
}

/// Text boxes for typed answers, made once with the panel (a form uses one per question).
pub fn make_inputs(window: &mut Window, cx: &mut Context<Workbench>) -> Vec<Entity<InputState>> {
    (0..6).map(|_| cx.new(|cx| InputState::new(window, cx).placeholder(tr("Type your answer")))).collect()
}

#[cfg(test)]
mod tests {
    use super::{answer, letter, parse_schema, Kind, OTHER};
    use serde_json::json;

    #[test]
    fn a_choice_question_and_its_answers() {
        let schema = json!({
            "type": "object",
            "properties": {
                "db": { "type": "string", "title": "Which database?", "oneOf": [
                    { "const": "pg", "title": "PostgreSQL", "description": "Fast, needs a server" },
                    { "const": "sqlite", "title": "SQLite" } ] },
                "tests": { "type": "boolean", "title": "Add tests?" },
                "extras": { "type": "array", "items": { "enum": ["docker", "ci"] } },
                "port": { "type": "integer" }
            },
            "required": ["db"]
        });
        let f = parse_schema(&schema);
        assert_eq!(f.len(), 4);
        let db = f.iter().position(|x| x.key == "db").unwrap();
        assert!(matches!(&f[db].kind, Kind::Choice { options, multi: false } if options.len() == 2 && options[0].description == "Fast, needs a server"));
        let idx = |k: &str| f.iter().position(|x| x.key == k).unwrap();
        let mut picked = vec![Vec::new(); 4];
        let mut typed = vec![String::new(); 4];
        // Required question unanswered.
        assert_eq!(answer(&f, &picked, &typed).unwrap_err(), "Which database?");
        picked[idx("db")] = vec![1];
        picked[idx("tests")] = vec![0];
        picked[idx("extras")] = vec![0, OTHER];
        typed[idx("extras")] = "k8s".into();
        typed[idx("port")] = "8080".into();
        let a = answer(&f, &picked, &typed).unwrap();
        assert_eq!(a["db"], "sqlite");
        assert_eq!(a["tests"], true);
        assert_eq!(a["extras"], json!(["docker", "k8s"]));
        assert_eq!(a["port"], 8080);
        // Typed answer instead of an option.
        picked[idx("db")] = vec![OTHER];
        typed[idx("db")] = "DuckDB".into();
        assert_eq!(answer(&f, &picked, &typed).unwrap()["db"], "DuckDB");
        // A number that isn't one names its question.
        typed[idx("port")] = "eighty".into();
        assert_eq!(answer(&f, &picked, &typed).unwrap_err(), "port");
    }

    #[test]
    fn letters_run_past_z() {
        assert_eq!(letter(0), "A");
        assert_eq!(letter(3), "D");
        assert_eq!(letter(26), "AA");
    }
}
