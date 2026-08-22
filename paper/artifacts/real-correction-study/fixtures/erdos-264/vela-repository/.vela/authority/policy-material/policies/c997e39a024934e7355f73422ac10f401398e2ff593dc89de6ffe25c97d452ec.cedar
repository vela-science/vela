
permit (
    principal == Human::"local:device-sha256:67fbb8e56377e6868e9f941524e0bf39cfb4fd2a4bfdd25c2edb93fc82f86213|uid:501",
    action in [
        Action::"authority_initialize",
        Action::"authority_model_migrate",
        Action::"authority_rotate",
        Action::"authority_close",
        Action::"policy_rotate",
        Action::"review_accept",
        Action::"review_reject"
    ],
    resource
) when {
    context.exact &&
    !context.authentication.recovery_recent
};

permit(principal, action == Action::"work_claim", resource)
when {
    context.exact &&
    context.authentication.method == "agent_event_signature" &&
    context.authentication.assurance == "single_factor"
};

permit(principal, action == Action::"submission_register", resource)
when {
    context.exact &&
    context.authentication.method == "agent_record_signature" &&
    context.authentication.assurance == "single_factor"
};

permit(principal, action == Action::"verification_import", resource)
when {
    context.exact &&
    context.authentication.method == "agent_record_signature" &&
    context.authentication.assurance == "single_factor"
};
