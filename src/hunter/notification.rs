// crates.io
use lettre::{
	message::Mailbox, transport::smtp::authentication::Credentials, Message, SmtpTransport,
	Transport,
};
use serde::Serialize;
// slothunter
use crate::hunter::*;

#[derive(Debug)]
pub struct Mail {
	pub sender: Sender,
	pub receivers: Vec<Mailbox>,
}
#[derive(Debug)]
pub struct Sender {
	pub username: Mailbox,
	pub password: String,
	pub smtp: String,
}

impl Hunter {
	pub fn notify_mail<S>(&self, object: &S, addition: &str)
	where
		S: Serialize,
	{
		if let Some(m) = &self.configuration.notification.mail {
			let notification = serde_json::to_string(&serde_json::json!({
				"object": object,
				"addition": addition,
			}))
			.expect("json must be valid");

			for to in &m.receivers {
				let mail = Message::builder()
					.from(m.sender.username.clone())
					.to(to.to_owned())
					.subject("Slothunter")
					.body(notification.clone())
					.expect("message must be valid");
				let smtp_transport = SmtpTransport::relay(&m.sender.smtp)
					.expect("smtp must be valid")
					.credentials(Credentials::new(
						m.sender.username.email.to_string(),
						m.sender.password.clone(),
					))
					.build();

				let _ = smtp_transport.send(&mail);
			}
		}
	}

	pub async fn notify_webhook<S>(&self, object: &S, addition: &str)
	where
		S: Serialize,
	{
		let json = serde_json::json!({
			"object": object,
			"addition": addition,
		});

		for u in &self.configuration.notification.webhooks {
			if u.starts_with("https://hooks.slack.com/services/") {
				let _ = self
					.http
					.post(u)
					.json(&serde_json::json!({
						"text": serde_json::to_string(&json).unwrap(),
					}))
					.send()
					.await;
			} else {
				let _ = self.http.post(u).json(&json).send().await;
			}
		}
	}
}
