use std::collections::HashMap;

use clap::ArgAction;
use clap::Parser;
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

#[derive(Debug, Parser)]
struct Args {
    #[arg(short='f', long="field", num_args = 2, action = ArgAction::Append, value_names = ["NAME", "VALUE"])]
    fields: Vec<Vec<String>>,

    #[arg(value_name = "KEY")]
    issue_key: issue::Key,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    dbg!(&args);

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
    dbg!(&input_fields);

    let issue = get_issue(&args.issue_key)?;
    dbg!(&issue);

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
                        issue: args.issue_key.clone(),
                        field: field.name().clone(),
                    })?;

            if issue_value != field.value() {
                field_updates.insert(field.name(), field.value());
            }

            Ok(())
        })?;
    dbg!(&field_updates);

    let source = r###"Changed field{{ fields|pluralize}} {% for name, value in fields|items %}`{{ name }}`{% if not loop.last %}, {% endif %}{% endfor %}."###;
    let mut env = Environment::new();
    env.add_filter("pluralize", minijinja_contrib::filters::pluralize);
    env.add_template("comment", source)?;
    let tmpl = env.get_template("comment")?;
    let comment = tmpl.render(context! {fields => field_updates})?;

    //if let Some(comment) = Editor::new().extension(".txt").edit(&comment)? {}
    let _comment: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Comment for updated fields?")
        .with_initial_text(comment)
        .interact_text()?;

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
