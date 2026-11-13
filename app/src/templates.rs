//! Ready-made screens: a template is a screen's widgets, variables,
//! components and events written in the same JSON the MCP tools take, so
//! the same files can later come from an online store.

use crate::mcp;
use crate::model::*;
use serde_json::{json, Value};

pub struct Template {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// The screen name it suggests.
    pub suggested: &'static str,
    pub icon: &'static str,
    spec: fn() -> Value,
}

const INK: &str = "#1C1C28";
const MUTED: &str = "#6B6B80";
const BRAND: &str = "#5B5BD6";

fn heading(id: &str, text: &str) -> Value {
    json!({"type": "text", "id": id, "text": text, "text_size": 28, "bold": true, "text_color": INK})
}
fn note(id: &str, text: &str) -> Value {
    json!({"type": "text", "id": id, "text": text, "text_size": 15, "text_color": MUTED, "margin": 2})
}
fn field(id: &str, hint: &str, password: bool) -> Value {
    json!({"type": "text_field", "id": id, "hint": hint, "password": password, "width": "match_parent", "text_size": 16, "margin": 4})
}
fn primary(id: &str, text: &str) -> Value {
    json!({"type": "button", "id": id, "text": text, "width": "match_parent", "text_size": 16, "background": BRAND, "text_color": "#FFFFFF", "margin": 6})
}
fn link(id: &str, text: &str) -> Value {
    json!({"type": "text", "id": id, "text": text, "text_size": 14, "text_color": BRAND, "bold": true})
}
fn page(children: Vec<Value>) -> Value {
    json!({"type": "column", "id": "page", "width": "match_parent", "height": "match_parent", "padding": 24, "background": "#FFFFFF", "children": children})
}
fn toast(t: &str) -> Value {
    json!({"op": "doToast", "args": [t]})
}
fn blank(id: &str) -> Value {
    json!({"op": "stringEquals", "args": [{"op": "trim", "args": [{"op": "getText", "args": [id]}]}, ""]})
}
fn or(a: Value, b: Value) -> Value {
    json!({"op": "||", "args": [a, b]})
}

fn text_of(id: &str) -> Value {
    json!({"op": "getText", "args": [id]})
}
fn set_text(id: &str, v: Value) -> Value {
    json!({"op": "setText", "args": [id, v]})
}
fn join(a: Value, b: Value) -> Value {
    json!({"op": "stringJoin", "args": [a, b]})
}

pub static TEMPLATES: &[Template] = &[
    Template {
        id: "login",
        name: "Log in",
        description: "Email and password, with links to sign up and reset the password",
        suggested: "login",
        icon: crate::icons::SMARTPHONE,
        spec: || {
            json!({
                "widgets": [page(vec![
                    heading("title", "Welcome back"),
                    note("subtitle", "Log in to continue"),
                    json!({"type": "column", "id": "form", "width": "match_parent", "height": "wrap_content", "padding": 0, "children": [
                        field("email", "Email", false),
                        field("password", "Password", true),
                        primary("logIn", "Log in"),
                    ]}),
                    json!({"type": "row", "id": "links", "width": "match_parent", "children": [
                        link("forgot", "Forgot password?"),
                        json!({"type": "text", "id": "gap", "text": "", "weight": 1}),
                        link("signUp", "Create an account"),
                    ]}),
                ])],
                "events": [
                    {"target": "logIn", "event": "onClick", "blocks": [
                        {"op": "ifElse", "args": [or(blank("email"), blank("password"))],
                         "do": [toast("Enter your email and password")],
                         "else": [{"op": "ifElse", "args": [{"op": "not", "args": [{"op": "stringContains", "args": [{"op": "getText", "args": ["email"]}, "@"]}]}],
                                   "do": [toast("That email doesn't look right")],
                                   "else": [toast("Logging in…")]}]}
                    ]},
                    {"target": "forgot", "event": "onClick", "blocks": [toast("Password reset isn't set up yet")]},
                    {"target": "signUp", "event": "onClick", "blocks": [toast("Sign-up isn't set up yet")]},
                ],
            })
        },
    },
    Template {
        id: "supabase_login",
        name: "Log in with Supabase",
        description: "Sign in or create an account with your linked Supabase project; stays signed in",
        suggested: "login",
        icon: crate::icons::DATABASE,
        spec: || {
            let signed_in = json!([
                set_text("status", json!("")),
                set_text("subtitle", join(json!("Signed in as "), json!({"op": "supaUserEmail", "args": []}))),
            ]);
            let mut welcome = signed_in.as_array().unwrap().clone();
            welcome.push(toast("Welcome!"));
            let failed = json!([set_text("status", json!({"op": "supaError", "args": []}))]);
            let check = |then: Vec<Value>| json!({"op": "ifElse", "args": [or(blank("email"), blank("password"))],
                "do": [set_text("status", json!("Enter your email and password"))],
                "else": then});
            json!({
                "widgets": [page(vec![
                    heading("title", "Welcome"),
                    note("subtitle", "Sign in or create an account"),
                    json!({"type": "column", "id": "form", "width": "match_parent", "height": "wrap_content", "padding": 0, "children": [
                        field("email", "Email", false),
                        field("password", "Password (6 or more letters)", true),
                        primary("signIn", "Sign in"),
                        json!({"type": "button", "id": "signUp", "text": "Create account", "width": "match_parent", "text_size": 16, "background": "#EEEEF8", "text_color": BRAND, "margin": 6}),
                    ]}),
                    json!({"type": "text", "id": "status", "text": "", "text_size": 14, "text_color": "#C62828", "margin": 6}),
                ])],
                "events": [
                    {"target": "screen", "event": "onCreate", "blocks": [
                        {"op": "if", "args": [{"op": "supaSignedIn", "args": []}], "do": signed_in}
                    ]},
                    {"target": "signIn", "event": "onClick", "blocks": [check(vec![
                        set_text("status", json!("Signing in…")),
                        json!({"op": "supaSignIn", "args": [text_of("email"), text_of("password")], "do": welcome, "else": failed}),
                    ])]},
                    {"target": "signUp", "event": "onClick", "blocks": [check(vec![
                        set_text("status", json!("Creating your account…")),
                        json!({"op": "supaSignUp", "args": [text_of("email"), text_of("password")],
                               "do": [{"op": "ifElse", "args": [{"op": "supaSignedIn", "args": []}],
                                       "do": welcome,
                                       "else": [set_text("status", json!("Check your email to confirm your account, then sign in"))]}],
                               "else": failed}),
                    ])]},
                ],
            })
        },
    },
    Template {
        id: "signup",
        name: "Sign up",
        description: "Name, email and a password typed twice, all checked",
        suggested: "signup",
        icon: crate::icons::PENCIL,
        spec: || {
            json!({
                "widgets": [page(vec![
                    heading("title", "Create your account"),
                    note("subtitle", "It only takes a minute"),
                    field("name", "Name", false),
                    field("email", "Email", false),
                    field("password", "Password (6 or more letters)", true),
                    field("confirm", "Password again", true),
                    primary("create", "Create account"),
                    json!({"type": "row", "id": "links", "width": "match_parent", "gravity": "center", "children": [
                        note("have", "Already have an account?"),
                        link("logIn", "Log in"),
                    ]}),
                ])],
                "events": [
                    {"target": "create", "event": "onClick", "blocks": [
                        {"op": "ifElse", "args": [or(blank("name"), or(blank("email"), blank("password")))],
                         "do": [toast("Fill in every field")],
                         "else": [{"op": "ifElse", "args": [{"op": "<", "args": [{"op": "stringLength", "args": [{"op": "getText", "args": ["password"]}]}, 6]}],
                                   "do": [toast("Use 6 or more letters for the password")],
                                   "else": [{"op": "ifElse", "args": [{"op": "not", "args": [{"op": "stringEquals", "args": [{"op": "getText", "args": ["password"]}, {"op": "getText", "args": ["confirm"]}]}]}],
                                             "do": [toast("The passwords don't match")],
                                             "else": [toast("Creating your account…")]}]}]}
                    ]},
                    {"target": "logIn", "event": "onClick", "blocks": [{"op": "finishActivity", "args": []}]},
                ],
            })
        },
    },
    Template {
        id: "settings",
        name: "Settings",
        description: "Switches that remember their choice on the phone",
        suggested: "settings",
        icon: crate::icons::SETTINGS,
        spec: || {
            let switch = |id: &str, text: &str| json!({"type": "switch", "id": id, "text": text, "width": "match_parent", "text_size": 16, "padding": 14});
            let save = |id: &str| {
                json!({"target": id, "event": "onCheckedChange", "blocks": [
                    {"op": "ifElse", "args": [{"op": "eventChecked", "args": []}],
                     "do": [{"op": "fileSetData", "args": ["prefs", id, "on"]}],
                     "else": [{"op": "fileSetData", "args": ["prefs", id, "off"]}]}
                ]})
            };
            let load = |id: &str, default_on: bool| {
                json!({"op": "ifElse", "args": [{"op": "fileContainsData", "args": ["prefs", id]}],
                       "do": [{"op": "setChecked", "args": [id, {"op": "stringEquals", "args": [{"op": "fileGetData", "args": ["prefs", id]}, "on"]}]}],
                       "else": [{"op": "setChecked", "args": [id, default_on]}]})
            };
            json!({
                "components": [{"type": "saved_data", "name": "prefs"}],
                "widgets": [page(vec![
                    heading("title", "Settings"),
                    note("subtitle", "Your choices are kept on this phone"),
                    switch("notifications", "Notifications"),
                    switch("sounds", "Sounds"),
                    switch("darkMode", "Dark mode"),
                    json!({"type": "text", "id": "version", "text": "Version 1.0", "text_size": 13, "text_color": MUTED, "margin": 12}),
                ])],
                "events": [
                    {"target": "screen", "event": "onCreate", "blocks": [load("notifications", true), load("sounds", true), load("darkMode", false)]},
                    save("notifications"), save("sounds"), save("darkMode"),
                ],
            })
        },
    },
    Template {
        id: "list",
        name: "List",
        description: "A list you can add to, with a message when an item is tapped",
        suggested: "items",
        icon: crate::icons::LIST,
        spec: || {
            let add = |t: &str| json!({"op": "addListStr", "args": [t, "names"]});
            json!({
                "variables": [{"name": "names", "type": "list_string"}],
                "widgets": [page(vec![
                    heading("title", "My list"),
                    json!({"type": "row", "id": "adder", "width": "match_parent", "vertical_gravity": "center", "children": [
                        json!({"type": "text_field", "id": "newItem", "hint": "New item", "weight": 1, "text_size": 16}),
                        json!({"type": "button", "id": "add", "text": "Add", "background": BRAND, "text_color": "#FFFFFF", "margin": 4}),
                    ]}),
                    json!({"type": "list", "id": "items", "width": "match_parent", "height": "match_parent", "weight": 1}),
                ])],
                "events": [
                    {"target": "screen", "event": "onCreate", "blocks": [add("Apples"), add("Bread"), add("Coffee"), {"op": "listSetData", "args": ["items", "names"]}]},
                    {"target": "add", "event": "onClick", "blocks": [
                        {"op": "if", "args": [{"op": "not", "args": [blank("newItem")]}], "do": [
                            add_text("newItem"),
                            {"op": "listSetData", "args": ["items", "names"]},
                            {"op": "setText", "args": ["newItem", ""]},
                        ]}
                    ]},
                    {"target": "items", "event": "onItemClicked", "blocks": [
                        {"op": "doToast", "args": [{"op": "stringJoin", "args": ["You picked ", {"op": "getAtListStr", "args": [{"op": "eventPosition", "args": []}, "names"]}]}]}
                    ]},
                ],
            })
        },
    },
];

fn add_text(id: &str) -> Value {
    json!({"op": "addListStr", "args": [{"op": "getText", "args": [id]}, "names"]})
}

pub fn find(id: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|t| t.id == id)
}

/// A new screen called `name` built from a template (or from JSON in the
/// same form, e.g. a file from the store). `screens`: the project's screen
/// activities, for blocks that open another screen.
pub fn build(spec: &Value, name: &str, screens: &crate::blocks::Choices) -> Result<Screen, String> {
    let mut s = Screen::new(name);
    s.events.clear();
    for v in spec.get("variables").and_then(|v| v.as_array()).into_iter().flatten() {
        let n = v.get("name").and_then(|n| n.as_str()).ok_or("A variable needs a name")?;
        match mcp::var_type(v.get("type").and_then(|t| t.as_str()).unwrap_or(""))? {
            Ok(ty) => s.vars.push(Var { name: n.into(), ty }),
            Err(ty) => s.lists.push(ListVar { name: n.into(), ty }),
        }
    }
    for c in spec.get("components").and_then(|v| v.as_array()).into_iter().flatten() {
        let kind = mcp::comp_kind(c.get("type").and_then(|t| t.as_str()).unwrap_or(""))?;
        let n = c.get("name").and_then(|n| n.as_str()).unwrap_or(kind.default_name());
        s.components.push(Component { name: n.into(), kind });
    }
    for w in spec.get("widgets").and_then(|v| v.as_array()).into_iter().flatten() {
        mcp::add_tree(&mut s, "root", usize::MAX, w, &mut vec![])?;
    }
    let mut screens = screens.clone();
    screens.screens.push(s.activity_name());
    for e in spec.get("events").and_then(|v| v.as_array()).into_iter().flatten() {
        let target = e.get("target").and_then(|t| t.as_str()).unwrap_or("");
        let target = if target == "screen" { "" } else { target };
        let name = e.get("event").and_then(|t| t.as_str()).ok_or("An event needs a name")?;
        let blocks = mcp::parse_stack(e.get("blocks"), &s, &screens, &format!("{target} {name}"))?;
        s.events.push(Event { target: target.into(), name: name.into(), blocks, loose: vec![], params: vec![] });
    }
    if !s.events.iter().any(|e| e.target.is_empty() && e.name == "onCreate") {
        s.events.insert(0, Event { target: String::new(), name: "onCreate".into(), blocks: vec![], loose: vec![], params: vec![] });
    }
    Ok(s)
}

/// A screen from one of the built-in templates.
pub fn screen(t: &Template, name: &str, screens: &crate::blocks::Choices) -> Result<Screen, String> {
    build(&(t.spec)(), name, screens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_runs() {
        for t in TEMPLATES {
            let s = screen(t, "main", &Default::default()).unwrap_or_else(|e| panic!("{}: {e}", t.id));
            assert!(!s.widgets.is_empty(), "{}", t.id);
            let mut p = Project::new(1, "T", "T", "com.my.t");
            p.screens[0] = s;
            // the code for both phones comes out, and the offline phone opens it
            let kt = crate::native::kotlin(&p, &p.screens[0]);
            assert!(kt.contains("class MainActivity"), "{}", t.id);
            let _ = crate::native::swift_screen(&p, &p.screens[0]);
            let rt = crate::runtime::Runtime::new(p, 0.0);
            assert!(rt.crash.is_none(), "{}", t.id);
        }
    }

    #[test]
    fn login_checks_the_fields() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens[0] = screen(find("login").unwrap(), "main", &Default::default()).unwrap();
        assert!(p.screens[0].widget("password").unwrap().password);
        let mut rt = crate::runtime::Runtime::new(p, 0.0);
        rt.fire("logIn", "onClick", Default::default());
        assert_eq!(rt.toasts.last().unwrap().0, "Enter your email and password");
        rt.widgets.get_mut("email").unwrap().text = "me@site.com".into();
        rt.widgets.get_mut("password").unwrap().text = "secret".into();
        rt.fire("logIn", "onClick", Default::default());
        assert_eq!(rt.toasts.last().unwrap().0, "Logging in…");
    }

    #[test]
    fn the_list_adds_items() {
        let mut p = Project::new(1, "T", "T", "com.my.t");
        p.screens[0] = screen(find("list").unwrap(), "main", &Default::default()).unwrap();
        let mut rt = crate::runtime::Runtime::new(p, 0.0);
        rt.widgets.get_mut("newItem").unwrap().text = "Dates".into();
        rt.fire("add", "onClick", Default::default());
        assert_eq!(rt.widgets["items"].items.len(), 4);
        assert_eq!(rt.widgets["newItem"].text, "");
    }
}
