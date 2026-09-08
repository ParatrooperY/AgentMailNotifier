use agent_mail_notifier_core::{
    DeliveryContext, DeliveryDecision, decide_delivery,
};

#[test]
fn delivers_only_when_the_running_app_has_a_ready_enabled_integration() {
    let ready = DeliveryContext {
        main_program_running: true,
        smtp_verified: true,
        integration_installed: true,
        enabled_preference: true,
    };

    assert_eq!(decide_delivery(ready), DeliveryDecision::Deliver);

    for unavailable in [
        DeliveryContext { main_program_running: false, ..ready },
        DeliveryContext { smtp_verified: false, ..ready },
        DeliveryContext { integration_installed: false, ..ready },
        DeliveryContext { enabled_preference: false, ..ready },
    ] {
        assert_eq!(
            decide_delivery(unavailable),
            DeliveryDecision::DropSilently,
        );
    }
}
