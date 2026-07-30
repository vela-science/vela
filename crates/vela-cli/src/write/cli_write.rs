use crate::cli_commands::ReviewAction;

pub(crate) fn cmd_review(action: ReviewAction) {
    match action {
        ReviewAction::Inbox { frontier, json } => {
            crate::decision_inbox::cmd_decision_inbox(&frontier, json)
        }
        ReviewAction::List {
            frontier,
            status,
            limit,
            cursor,
            json,
        } => crate::current_repository::cmd_current_review_list(
            &frontier,
            status.as_deref(),
            limit,
            cursor.as_deref(),
            json,
        ),
        ReviewAction::Show {
            frontier,
            proposal_id,
            json,
        } => crate::current_repository::cmd_current_review_show(&frontier, &proposal_id, json),
        ReviewAction::Accept {
            frontier,
            proposal_id,
            reason,
            json,
        } => crate::cli::review_decision::cmd_review_decide(
            frontier,
            &proposal_id,
            crate::current_repository_decision::DecisionAction::Accept,
            reason,
            json,
        ),
        ReviewAction::Reject {
            frontier,
            proposal_id,
            reason,
            json,
        } => crate::cli::review_decision::cmd_review_decide(
            frontier,
            &proposal_id,
            crate::current_repository_decision::DecisionAction::Reject,
            reason,
            json,
        ),
    }
}
