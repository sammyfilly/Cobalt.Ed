use std::cmp::Ordering;

use crate::cobalt_model::pagination_config::Include;
use crate::cobalt_model::pagination_config::PaginationConfig;
use crate::cobalt_model::permalink;
use crate::cobalt_model::slug;
use crate::cobalt_model::SortOrder;
use crate::document;
use crate::document::Document;
use crate::error::*;

mod categories;
mod helpers;
mod paginator;
mod tags;

use paginator::Paginator;

pub fn generate_paginators(
    doc: &mut Document,
    posts_data: &[liquid::value::Value],
) -> Result<Vec<Paginator>> {
    let config = doc
        .front
        .pagination
        .as_ref()
        .expect("Front should have pagination here.");
    let mut all_posts: Vec<_> = posts_data.iter().collect();
    match config.include {
        Include::All => {
            sort_posts(&mut all_posts, &config);
            create_all_paginators(&all_posts, &doc, &config, None)
        }
        Include::Tags => tags::create_tags_paginators(&all_posts, &doc, &config),
        Include::Categories => categories::create_categories_paginators(&all_posts, &doc, &config),
        Include::None => {
            unreachable!("PaginationConfigBuilder should have lead to a None for pagination.")
        }
    }
}

fn create_all_paginators(
    all_posts: &[&liquid::value::Value],
    doc: &Document,
    pagination_cfg: &PaginationConfig,
    index_title: Option<&liquid::value::Value>,
) -> Result<Vec<Paginator>> {
    let total_pages = all_posts.len();
    // f32 used here in order to not lose information to ceil the result,
    // otherwise we can lose an index
    let total_indexes = (total_pages as f32 / pagination_cfg.per_page as f32).ceil() as usize;
    let paginators: Result<Vec<_>> = all_posts
        .chunks(pagination_cfg.per_page as usize)
        .enumerate()
        .map(|(i, chunk)| {
            paginator::create_paginator(
                i,
                total_indexes,
                total_pages,
                &pagination_cfg,
                &doc,
                &chunk,
                index_title,
            )
        })
        .collect();
    paginators
}

// sort posts by multiple criteria
fn sort_posts(posts: &mut Vec<&liquid::value::Value>, config: &PaginationConfig) {
    let order: fn(&liquid::value::Scalar, &liquid::value::Scalar) -> Ordering = match config.order {
        SortOrder::Desc => {
            |a, b: &liquid::value::Scalar| b.partial_cmp(a).unwrap_or(Ordering::Equal)
        }
        SortOrder::Asc => {
            |a: &liquid::value::Scalar, b| a.partial_cmp(b).unwrap_or(Ordering::Equal)
        }
        SortOrder::None => {
            // when built, order is set like this:
            // `order.unwrap_or(SortOrder::Desc);` so it's unreachable
            unreachable!("Sort order should have default value when constructing PaginationConfig")
        }
    };
    posts.sort_by(|a, b| {
        let keys = &config.sort_by;
        let mut cmp = Ordering::Less;
        for k in keys {
            cmp = match (
                helpers::extract_scalar(a, &k),
                helpers::extract_scalar(b, &k),
            ) {
                (Some(a), Some(b)) => order(a, b),
                (None, None) => Ordering::Equal,
                (_, None) => Ordering::Greater,
                (None, _) => Ordering::Less,
            };
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        cmp
    })
}

fn pagination_attributes(page_num: i32, include: String) -> liquid::value::Object {
    let attributes: liquid::value::Object = vec![
        ("num".into(), liquid::value::Value::scalar(page_num)),
        ("include".into(), liquid::value::Value::scalar(include)),
    ]
    .into_iter()
    .collect();
    attributes
}

fn index_to_string(index: &liquid::value::Value) -> String {
    if let Some(index) = index.as_array() {
        let mut s: String = index
            .iter()
            .map(|i| {
                let mut s = slug::slugify(i.to_str().to_string());
                s.push('/');
                s
            })
            .collect();
        s.pop(); // remove last '/'
        s
    } else {
        slug::slugify(index.to_str().to_string())
    }
}

fn interpret_permalink(
    config: &PaginationConfig,
    doc: &Document,
    page_num: usize,
    index: Option<&liquid::value::Value>,
) -> Result<String> {
    Ok(if page_num == 1 {
        if let Some(index) = index {
            let include: &str = config.include.into();
            format!("{}/{}", include, index_to_string(&index))
        } else {
            doc.url_path.clone()
        }
    } else {
        let mut attributes = document::permalink_attributes(&doc.front, &doc.file_path);
        let include: &str = config.include.into();
        let include: String = if let Some(index) = index {
            format!("{}/{}", include, index_to_string(&index))
        } else {
            include.to_owned()
        };
        let pagination_attr = pagination_attributes(page_num as i32, include);
        attributes.extend(pagination_attr.into_iter());
        permalink::explode_permalink(&config.permalink, &attributes)?
    })
}