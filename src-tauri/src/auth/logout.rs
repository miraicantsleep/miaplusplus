//! Submit ITS sign-out and verify central-session revocation.
use super::{
    AuthResult, Session,
    apps::{self, Connection, Context, read_body},
};
use scraper::{Html, Selector};
use std::sync::Arc;
use tauri::Url;
use tokio_util::sync::CancellationToken;

const SSO: &str = "https://my.its.ac.id/";
const FAILED: &str =
    "ITS sign-out could not be confirmed. Your saved session was kept so you can retry Log out.";

fn endpoint(url: &Url, path: &str) -> bool {
    url.origin().ascii_serialization() == "https://my.its.ac.id"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.path() == path
}
fn confirmation(body: &str, source: &Url) -> AuthResult<(Url, Vec<(String, String)>)> {
    if !endpoint(source, "/logout") {
        return Err(FAILED.into());
    }
    let params = source.query_pairs().collect::<Vec<_>>();
    if params.len() != 1 || params[0].0 != "logout_request_id" || params[0].1.is_empty() {
        return Err(FAILED.into());
    }
    let document = Html::parse_document(body);
    let inputs = Selector::parse("input").unwrap();
    for form in document.select(&Selector::parse("form").unwrap()) {
        let fields = form
            .select(&inputs)
            .filter_map(|input| {
                Some((
                    input.value().attr("name")?.to_owned(),
                    input.value().attr("value")?.to_owned(),
                ))
            })
            .collect::<Vec<_>>();
        if !fields
            .iter()
            .any(|(name, value)| name == "action" && value == "yes")
        {
            continue;
        }
        let target = form
            .value()
            .attr("action")
            .and_then(|action| source.join(action).ok())
            .ok_or(FAILED)?;
        if !form
            .value()
            .attr("method")
            .is_some_and(|method| method.eq_ignore_ascii_case("post"))
            || !endpoint(&target, "/terminate-session")
            || target.query().is_some()
            || fields.len() != 2
            || fields
                .iter()
                .filter(|(name, value)| {
                    name == "logout_request_id" && value == &params[0].1 && value.len() <= 1024
                })
                .count()
                != 1
        {
            return Err(FAILED.into());
        }
        return Ok((target, fields));
    }
    Err(FAILED.into())
}
pub(super) async fn invalidate(
    original: Arc<Session>,
    cancel: CancellationToken,
) -> AuthResult<()> {
    let context = &Context::new(cancel);
    let connection = Connection::new(&original, SSO)?;
    let mut response = connection
        .get(Url::parse(SSO).unwrap().join("signout").unwrap(), context)
        .await?;
    for _ in 0..4 {
        let source = response.url().clone();
        if response.status().as_u16() == 200 {
            let body = read_body(response, context).await?;
            let (target, fields) = confirmation(&body, &source)?;
            let result = connection
                .post_form(target, &source, &fields, context)
                .await?;
            if !result.status().is_success() && !result.status().is_redirection() {
                return Err(FAILED.into());
            }
            break;
        }
        let target = response
            .headers()
            .get("location")
            .and_then(|location| location.to_str().ok())
            .and_then(|location| source.join(location).ok());
        match target {
            Some(target) if response.status().is_redirection() && endpoint(&target, "/logout") => {
                response = connection.get(target, context).await?;
            }
            _ => break,
        }
    }
    if apps::central_rejected(&original, context).await? {
        Ok(())
    } else {
        Err("ITS still accepts the saved session. Please retry Log out.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn form(target: &str, fields: &str) -> String {
        format!(
            r#"<form method="POST" action="{target}"><input name="action" value="yes">{fields}</form>"#
        )
    }
    #[test]
    fn signout_posts_only_the_bound_confirmation_to_its() {
        let source = Url::parse("https://my.its.ac.id/logout?logout_request_id=test-id").unwrap();
        let field = r#"<input name="logout_request_id" value="test-id">"#;
        let (url, fields) = confirmation(&form("/terminate-session", field), &source).unwrap();
        assert_eq!(url.as_str(), "https://my.its.ac.id/terminate-session");
        assert!(
            fields
                .iter()
                .any(|(key, value)| key == "action" && value == "yes")
        );
        for target in [
            "https://evil.test/terminate-session",
            "http://my.its.ac.id/terminate-session",
            "/terminate-session?extra=1",
            "/other",
            "https://user@my.its.ac.id/terminate-session",
        ] {
            assert!(confirmation(&form(target, field), &source).is_err());
        }
        for fields in [
            "",
            r#"<input name="logout_request_id" value="wrong">"#,
            r#"<input name="logout_request_id" value="test-id"><input name="extra" value="1">"#,
        ] {
            assert!(confirmation(&form("/terminate-session", fields), &source).is_err());
        }
        assert!(
            confirmation(
                &form("/terminate-session", field).replace("POST", "GET"),
                &source
            )
            .is_err()
        );
        assert!(
            confirmation(
                &form("/terminate-session", field).replace("yes", "no"),
                &source
            )
            .is_err()
        );
    }
}
