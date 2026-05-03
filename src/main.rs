use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anstyle_progress::TermProgress;
use anstyle_progress::supports_term_progress;
use clap::ArgAction;
use clap::ValueHint;
use clap::{Parser, Subcommand};
use color_eyre::Result;
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use issue_cli_mockup::field::Field;
use issue_cli_mockup::field::Name;
use issue_cli_mockup::field::Value;
use issue_cli_mockup::issue;
use issue_cli_mockup::issue::Key;
use issue_cli_mockup::issue::get_issue;
use minijinja::Environment;
use minijinja::context;
use std::io::{self, IsTerminal};
use supports_hyperlinks::Stream::Stderr;
use tracing_human_layer::HumanLayer;
use tracing_indicatif::IndicatifLayer;
use tracing_indicatif::indicatif_eprintln;
use tracing_indicatif::span_ext::IndicatifSpanExt;
use tracing_indicatif::suspend_tracing_indicatif;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Edit, return or handover issues
#[derive(Debug, Parser)]
struct Args {
    #[command(subcommand)]
    command: Command,

    /// Config ⚙️ file
    #[arg(short = 'C', long = "config", env = "ISSUE_CONFIG", value_name = "FILE_PATH", value_hint = ValueHint::FilePath, global = true)]
    config: Option<PathBuf>,

    /// Field 📋 to update
    #[arg(short='f', long="field", num_args = 2, action = ArgAction::Append, value_names = ["NAME", "VALUE"], global = true)]
    fields: Vec<Vec<String>>,

    /// Comment 💬 for updated fields / transition
    #[arg(short = 'c', long = "comment", global = true)]
    comment: Option<String>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Edit issue ✏️
    ///
    /// Update fields + add comment
    Edit {
        /// Issue key 🪪; e.g. PRJ-42
        #[arg(value_name = "KEY")]
        issue_key: issue::Key,
    },
    /// Return issue ⏪
    ///
    /// Update fields + add comment + do transition status backwards
    Return {
        /// Issue key 🪪; e.g. PRJ-42
        #[arg(value_name = "KEY")]
        issue_key: issue::Key,
    },
    /// Handover issue ⏩
    ///
    /// Update fields + add comment + do transition status forward
    Handover {
        /// Issue key 🪪; e.g. PRJ-42
        #[arg(value_name = "KEY")]
        issue_key: issue::Key,
    },
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let indicatif_layer = IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(HumanLayer::new().with_output_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();

    tracing::debug!(?args, "Parsed command-line arguments.");

    let input_fields: Vec<Field> = args
        .fields
        .into_iter()
        .map(|field| {
            assert_eq!(field.len(), 2);
            let (name, value) = (field[0].clone(), field[1].clone());
            let (name, value) = (Name::new(name), Value::new(value));
            Field::new(name, value)
        })
        .collect();
    tracing::debug!(?input_fields, "Parsed fields input.");

    let issue_key = match args.command {
        Command::Edit { issue_key, .. }
        | Command::Return { issue_key, .. }
        | Command::Handover { issue_key } => issue_key,
    };

    let issue = get_issue(&issue_key)?;
    tracing::info!(key = %issue_key, ?issue, "Got issue.");

    #[derive(Debug, thiserror::Error)]
    #[error("could not find field {field} in issue {issue}")]
    struct FieldNotFoundError {
        issue: Key,
        field: Name,
    }

    let mut field_updates = HashMap::new();
    input_fields
        .iter()
        .try_for_each(|field| -> std::result::Result<(), FieldNotFoundError> {
            let issue_value =
                issue
                    .field_value(field.name())
                    .ok_or_else(|| FieldNotFoundError {
                        issue: issue_key.clone(),
                        field: field.name().clone(),
                    })?;

            if issue_value != field.value() {
                field_updates.insert(field.name(), field.value());
            }

            Ok(())
        })?;
    tracing::debug!(?field_updates, "Parsed issue field updates.");

    if !field_updates.is_empty() {
        let source = r###"Edited field{{ updates|pluralize}} {% for name, value in updates|items %}`{{ name }}`{% if not loop.last %}, {% endif %}{% endfor %}, FYI @{{ issue.reporter }}."###;
        let mut env = Environment::new();
        env.add_filter("pluralize", minijinja_contrib::filters::pluralize);
        env.add_template("comment", source)?;
        let tmpl = env.get_template("comment")?;
        tracing::debug!(tmpl = tmpl.source(), "Got comment template");
        tracing::trace!(vars = ?tmpl.undeclared_variables(true), "Got comment template undeclared variables");
        let ctx = context! {issue => issue, updates => field_updates};
        tracing::debug!(?ctx, "Prepared comment template context");
        let comment = tmpl.render(ctx)?;

        indicatif_eprintln!("\u{0007}\u{1F514}"); // 🔔
        let comment: String = suspend_tracing_indicatif(|| {
            //if let Some(comment) = Editor::new().extension(".txt").edit(&comment)? {}
            Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Comment for updated fields?")
                .with_initial_text(comment)
                .interact_text()
        })?;

        tracing::debug!(comment, "Got comment for issue field updates.");

        let _span = tracing::info_span!("edit_issue", key = %issue_key);
        _span.pb_start();

        let progress = TermProgress::start();
        if supports_term_progress(io::stderr().is_terminal()) {
            tracing::trace!("stderr supports term progress.");
            indicatif_eprintln!("{progress}");
        }

        std::thread::sleep(Duration::from_secs(3));

        let progress = TermProgress::remove();
        if supports_term_progress(io::stderr().is_terminal()) {
            indicatif_eprintln!("{progress}");
        }

        tracing::info!(key = %issue_key, "Edited issue.");
    }

    // TODO: ideally this would be emitted via tracing, but don't want unescaped ANSI from e.g. user input/fields
    if supports_hyperlinks::on(Stderr) {
        tracing::trace!("stderr supports term hyperlink.");

        let hyperlink = anstyle_hyperlink::Hyperlink::with_url(format!(
            "https://issues.example.com/{}",
            issue_key
        ));
        indicatif_eprintln!("Check {hyperlink}issue {}{hyperlink:#}", issue_key);
    } else {
        let hyperlink = format!("https://issues.example.com/{}", issue_key);
        indicatif_eprintln!("Check issue {}: {}", issue_key, hyperlink);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Args::command().debug_assert();
    }
}
