use gloo_net::http::Request;
use shared_types::Category;
use uuid::Uuid;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[function_component(CategoryManager)]
pub(crate) fn category_manager() -> Html {
    let categories = use_state(Vec::<Category>::new);
    let loading = use_state(|| true);
    let new_name = use_state(String::new);
    let new_color = use_state(|| "#3498db".to_string());
    let refresh_trigger = use_state(|| 0u32);

    // Fetch categories
    {
        let categories = categories.clone();
        let loading = loading.clone();
        let refresh_trigger = *refresh_trigger;

        use_effect_with(refresh_trigger, move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::get("/api/categories").send().await {
                    if response.ok() {
                        if let Ok(data) = response.json::<Vec<Category>>().await {
                            categories.set(data);
                        }
                    }
                }
                loading.set(false);
            });
            || ()
        });
    }

    let on_name_input = {
        let new_name = new_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_name.set(input.value());
        })
    };

    let on_color_input = {
        let new_color = new_color.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            new_color.set(input.value());
        })
    };

    let on_create = {
        let new_name = new_name.clone();
        let new_color = new_color.clone();
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            let name = (*new_name).clone();
            if name.is_empty() {
                return;
            }
            let color = (*new_color).clone();
            let new_name = new_name.clone();
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let body = serde_json::json!({
                    "name": name,
                    "color": color
                });
                if let Ok(response) = Request::post("/api/categories")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .unwrap()
                    .send()
                    .await
                {
                    if response.ok() {
                        new_name.set(String::new());
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    let delete_category = {
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |id: Uuid| {
            let refresh_trigger = refresh_trigger.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Ok(response) = Request::delete(&format!("/api/categories/{}", id))
                    .send()
                    .await
                {
                    if response.ok() {
                        refresh_trigger.set(*refresh_trigger + 1);
                    }
                }
            });
        })
    };

    if *loading {
        return html! {
            <div class="category-manager">
                <h2>{"Manage Categories"}</h2>
                <p class="loading">{"Loading categories..."}</p>
            </div>
        };
    }

    let categories_list = (*categories).clone();

    html! {
        <div class="category-manager">
            <h2>{"Manage Categories"}</h2>
            <div class="add-category">
                <form onsubmit={on_create}>
                    <input
                        type="text"
                        placeholder="Category name"
                        value={(*new_name).clone()}
                        oninput={on_name_input}
                    />
                    <input
                        type="color"
                        value={(*new_color).clone()}
                        oninput={on_color_input}
                    />
                    <button type="submit">{"Add Category"}</button>
                </form>
            </div>
            <div class="category-list">
                {if categories_list.is_empty() {
                    html! { <p class="empty-state">{"No categories yet!"}</p> }
                } else {
                    categories_list.iter().map(|category| {
                        let cat_id = category.id;
                        let delete = delete_category.clone();
                        html! {
                            <div key={category.id.to_string()} class="category-item">
                                <div
                                    class="category-color"
                                    style={format!("background-color: {}", category.color.as_deref().unwrap_or("#cccccc"))}
                                />
                                <span class="category-name">{&category.name}</span>
                                <button class="delete-btn" onclick={Callback::from(move |_| delete.emit(cat_id))}>{"Delete"}</button>
                            </div>
                        }
                    }).collect::<Html>()
                }}
            </div>
        </div>
    }
}
