#![recursion_limit="128"]
#![feature(proc_macro)]
#![feature(type_ascription)]

extern crate strum;
#[macro_use]
extern crate strum_macros;
#[macro_use]
extern crate serde_derive;

#[macro_use]  // json!{ } ??
extern crate serde_json;

#[macro_use]
extern crate yew;

#[macro_use]
extern crate stdweb;
extern crate failure;

use std::time::Duration;

use strum::IntoEnumIterator;

use yew::format::Json;
use yew::services::storage::{StorageService, Area};
use yew::services::fetch::{FetchService, Request, Response, StatusCode};
use yew::services::interval::{IntervalService};
use yew::html::*;
use yew::prelude::*;

const KEY: &'static str = "yew.todomvc.self";

struct Context {
    storage: StorageService,
    backend: TodoService,
    interval: IntervalService,
}

#[derive(Serialize, Deserialize)]
struct Model {
    entries: Vec<Entry>,
    filter: Filter,
    value: String,
    edit_value: String,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct Entry {
    description: String,
    completed: bool,
    // When editing true, set "editing" class, also li becomes input field
    editing: bool,
}


enum Msg {
    Add,
    Edit(usize),
    Update(String),
    UpdateAll(Vec<Entry>),
    UpdateEdit(String),
    Remove(usize),
    SetFilter(Filter),
    Tick,
    ToggleAll,
    ToggleEdit(usize),
    Toggle(usize),
    ClearCompleted,
    Nope,
}

impl Component<Context> for Model {
    type Msg = Msg;
    type Properties = ();

    fn create(_: Self::Properties, context: &mut Env<Context, Self>) -> Self {
        // Start background interval
        let interval_cb = context.send_back(|_| Msg::Tick);
        context.interval.spawn(Duration::from_secs(10), interval_cb);

        // fetch the canonical state from the server...
        let fetch_cb = context.send_back(|resp: Response<Json<Result<Vec<Entry>, failure::Error>>>| {
            let (meta, Json(result)) = resp.into_parts();
            if meta.status.is_success() {
                Msg::UpdateAll(result.expect("error retrieving result"))
            } else {
                js! { console.log("fetching all tasks failed") };
                Msg::Nope
            }
        });
        let req = Request::get("http://localhost:8000/tasks").body(None).unwrap();
        context.backend.api.fetch(req, fetch_cb);

        // ...but load from local storage until we fire the callback
        if let Json(Ok(restored_model)) = context.storage.restore(KEY) {
            restored_model
        } else {
            // ... or just make an empty list.
            Model {
                entries: Vec::new(),
                filter: Filter::All,
                value: "".into(),
                edit_value: "".into(),
            }
        }
    }

    fn update(&mut self, msg: Self::Msg, context: &mut Env<Context, Self>) -> ShouldRender {
        match msg {
            Msg::Add => {
                let entry = Entry {
                    description: self.value.clone(),
                    completed: false,
                    editing: false,
                };
                self.entries.push(entry.clone());
                self.value = "".to_string();
                let cb = context.send_back(|resp: Response<Json<Result<String, failure::Error>>>| {
                    let code: StatusCode = resp.status();
                    if code.is_success() {
                        println!("success");
                        Msg::Nope
                    } else {
                        println!("fail");
                        Msg::Nope
                    }
                });
                let body = serde_json::to_string(&entry).unwrap();
                let req = Request::post("http://localhost:8000/task")
                    .header("Content-Type", "application/json")
                    .body(body)
                    .expect("could not build request");
                context.backend.api.fetch(req, cb);
            }
            Msg::Edit(idx) => {
                let edit_value = self.edit_value.clone();
                self.complete_edit(idx, edit_value);
                self.edit_value = "".to_string();
            }
            Msg::Tick => {
                let entries = self.entries.clone();
                let cb = context.send_back(|resp: Response<Json<Result<String, failure::Error>>>| {
                    let code: StatusCode = resp.status();
                    if code.is_success() {
                        js! { console.log("sync success") };
                        Msg::Nope
                    } else {
                        Msg::Nope
                    }
                });
                let body = serde_json::to_string(&entries).unwrap();
                let req = Request::post("http://localhost:8000/tasks")
                    .header("Content-Type", "application/json")
                    .body(body)
                    .expect("could not build request");
                context.backend.api.fetch(req, cb);
            }
            Msg::Update(val) => {
                println!("Input: {}", val);
                js! { console.log("input:", @{val.clone()}) };
                self.value = val;
            }
            Msg::UpdateAll(vals) => {
                js! { console.log("got all tasks:") };
                self.entries = vals;
            }
            Msg::UpdateEdit(val) => {
                println!("Input: {}", val);
                self.edit_value = val;
            }
            Msg::Remove(idx) => {
                self.remove(idx);
            }
            Msg::SetFilter(filter) => {
                self.filter = filter;
            }
            Msg::ToggleEdit(idx) => {
                self.edit_value = self.entries[idx].description.clone();
                self.toggle_edit(idx);
            }
            Msg::ToggleAll => {
                let status = !self.is_all_completed();
                self.toggle_all(status);
            }
            Msg::Toggle(idx) => {
                self.toggle(idx);
            }
            Msg::ClearCompleted => {
                self.clear_completed();
            }
            Msg::Nope => {}
        }
        // We are serializable as JSON, and we store ourselves in local storage
        // on every update.
        context.storage.store(KEY, Json(&self));
        true
    }
}

impl Renderable<Context, Model> for Model {
    fn view(&self) -> Html<Context, Self> {
        html! {
            <div class="todomvc-wrapper",>
                <section class="todoapp",>
                    <header class="header",>
                        <h1>{ "todos!" }</h1>
                        { self.view_input() } // make new thing
                    </header>
                    <section class="main",>
                        <input class="toggle-all", type="checkbox", checked=self.is_all_completed(), onclick=|_| Msg::ToggleAll, />
                        <ul class="todo-list",>
                            { for self.entries.iter().filter(|e| self.filter.fit(e)).enumerate().map(view_entry) }
                        </ul>
                    </section>
                    <footer class="footer",>
                        <span class="todo-count",>
                            <strong>{ self.total() }</strong>
                            { " item(s) left" }
                        </span>
                        <ul class="filters",>
                            { for Filter::iter().map(|flt| self.view_filter(flt)) }
                        </ul>
                        <button class="clear-completed", onclick=|_| Msg::ClearCompleted,>
                            { format!("Clear completed ({})", self.total_completed()) }
                        </button>
                    </footer>
                </section>
                <footer class="info",>
                    <p>{ "Double-click to edit a todo" }</p>
                    <p>{ "Written by " }<a href="https://github.com/DenisKolodin/", target="_blank",>{ "Denis Kolodin" }</a></p>
                    <p>{ "Part of " }<a href="http://todomvc.com/", target="_blank",>{ "TodoMVC" }</a></p>
                </footer>
            </div>
        }
    }
}

impl Model {
    fn view_filter(&self, filter: Filter) -> Html<Context, Model> {
        let flt = filter.clone();
        html! {
            <li>
                <a class=if self.filter == flt { "selected" } else { "not-selected" },
                   href=&flt,
                   onclick=move |_| Msg::SetFilter(flt.clone()),>
                    { filter }
                </a>
            </li>
        }
    }

    fn view_input(&self) -> Html<Context, Model> {
        html! {
            // You can use standard Rust comments. One line:
            // <li></li>
            <input class="new-todo",
                   placeholder="What needs to be done?",
                   value=&self.value,
                   oninput=|e: InputData| Msg::Update(e.value),
                   onkeypress=|e: KeyData| {
                       if e.key == "Enter" { Msg::Add } else { Msg::Nope }
                   }, />
            /* Or multiline:
            <ul>
                <li></li>
            </ul>
            */
        }
    }
}

fn view_entry((idx, entry): (usize, &Entry)) -> Html<Context, Model> {
    html! {
        <li class=if entry.editing == true { "editing" } else { "" },>
            <div class="view",>
                <input class="toggle", type="checkbox", checked=entry.completed, onclick=move|_| Msg::Toggle(idx), />
                <label ondoubleclick=move|_| Msg::ToggleEdit(idx),>{ &entry.description }</label>
                <button class="destroy", onclick=move |_| Msg::Remove(idx), />
            </div>
            { view_entry_edit_input((idx, &entry)) }
        </li>
    }
}

fn view_entry_edit_input((idx, entry): (usize, &Entry)) -> Html<Context, Model> {
    if entry.editing == true {
        html! {
            <input class="edit",
                   type="text",
                   value=&entry.description,
                   oninput=|e: InputData| Msg::UpdateEdit(e.value),
                   onblur=move|_| Msg::Edit(idx),
                   onkeypress=move |e: KeyData| {
                      if e.key == "Enter" { Msg::Edit(idx) } else { Msg::Nope }
                   }, />
        }
    } else {
        html! { <input type="hidden", /> }
    }
}


fn main() {
    yew::initialize();
    let context = Context {
        storage: StorageService::new(Area::Local),
        backend: TodoService::new(),
        interval: IntervalService::new(),
    };
    let app: App<_, Model> = App::new(context);
    app.mount_to_body();
    yew::run_loop();
}

pub struct TodoService {
    api: FetchService,
}

impl TodoService {
    pub fn new() -> Self {
        Self {
            api: FetchService::new(),
        }
    }
}

#[derive(EnumIter, ToString, Clone, PartialEq)]
#[derive(Serialize, Deserialize)]
enum Filter {
    All,
    Active,
    Completed,
}

impl<'a> Into<Href> for &'a Filter {
    fn into(self) -> Href {
        match *self {
            Filter::All => "#/".into(),
            Filter::Active => "#/active".into(),
            Filter::Completed => "#/completed".into(),
        }
    }
}

impl Filter {
    fn fit(&self, entry: &Entry) -> bool {
        match *self {
            Filter::All => true,
            Filter::Active => !entry.completed,
            Filter::Completed => entry.completed,
        }
    }
}

impl Model {
    fn total(&self) -> usize {
        self.entries.len()
    }

    fn total_completed(&self) -> usize {
        self.entries.iter().filter(|e| Filter::Completed.fit(e)).count()
    }

    fn is_all_completed(&self) -> bool {
        let mut filtered_iter = self.entries
            .iter()
            .filter(|e| self.filter.fit(e))
            .peekable();

        if filtered_iter.peek().is_none() {
            return false;
        }

        filtered_iter.all(|e| e.completed)
    }

    fn toggle_all(&mut self, value: bool) {
        for entry in self.entries.iter_mut() {
            if self.filter.fit(entry) {
                entry.completed = value;
            }
        }
    }

    fn clear_completed(&mut self) {
        let entries = self.entries.drain(..)
            .filter(|e| Filter::Active.fit(e))
            .collect();
        self.entries = entries;
    }

    fn toggle(&mut self, idx: usize) {
        let filter = self.filter.clone();
        let mut entries = self.entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.completed = !entry.completed;
    }

    fn toggle_edit(&mut self, idx: usize) {
        let filter = self.filter.clone();
        let mut entries = self.entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.editing = !entry.editing;
    }

    fn complete_edit(&mut self, idx: usize, val: String) {
        let filter = self.filter.clone();
        let mut entries = self.entries
            .iter_mut()
            .filter(|e| filter.fit(e))
            .collect::<Vec<_>>();
        let entry = entries.get_mut(idx).unwrap();
        entry.description = val;
        entry.editing = !entry.editing;
    }

    fn remove(&mut self, idx: usize) {
        let idx = {
            let filter = self.filter.clone();
            let entries = self.entries
                .iter()
                .enumerate()
                .filter(|&(_, e)| filter.fit(e))
                .collect::<Vec<_>>();
            let &(idx, _) = entries.get(idx).unwrap();
            idx
        };
        self.entries.remove(idx);
    }
}
