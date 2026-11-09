//! Input routing and geometry must not wait for a renderer acquisition.

mod support;

use std::{cell::Cell, rc::Rc};

use zgui_geom::{CssPx, Point};
use zgui_platform::{PlatformCx, SurfaceEvent};
use zgui_reactive::{
    RwSignal,
    prelude::{Get, GetUntracked, Set},
};
use zgui_view::{AnyView, BuildCx, FocusTrapOptions, IntoView, NodeRef, View, ViewHost};
use zgui_vocab::{
    KeyCode, KeyEvent, KeyState, Modifiers, NamedKey, PhysicalKey, PointerAction, PointerEvent,
    Timestamp,
};

fn settle(app: &mut zgui_platform_headless::Harness<zgui_runtime::Runtime>) {
    let clock = app.platform().clock();
    app.app_mut().windows_mut()[0].settle(clock.as_ref());
}

fn draws(app: &zgui_platform_headless::Harness<zgui_runtime::Runtime>) -> u64 {
    app.app().windows()[0]
        .renderer()
        .as_any()
        .unwrap()
        .downcast_ref::<zgui_testkit_scene::CaptureRenderer>()
        .unwrap()
        .frames()
}

#[test]
fn input_settlement_mounts_and_focuses_controls_without_drawing() {
    let open = RwSignal::new(false);
    let trigger = NodeRef::new();
    let dialog = NodeRef::new();
    let inside = NodeRef::new();
    let keys = Rc::new(Cell::new(0));
    let heard = keys.clone();
    let mut app = support::app(
        "column { display: block; width: 400px; height: 300px } control { display: block; width: 100px; height: 24px }",
        move |cx: &mut BuildCx<'_>| {
            let heard = heard.clone();
            Box::new(
                zgui_elements::column()
                    .child(
                        zgui_elements::control()
                            .node_ref(trigger)
                            .on(zgui_view::events::POINTER_DOWN, move |_| open.set(true)),
                    )
                    .child(zgui_view::flow::Dynamic::new(move || {
                        if open.get() {
                            let heard = heard.clone();
                            AnyView::new(
                                zgui_elements::column().node_ref(dialog).child(
                                    zgui_elements::control()
                                        .node_ref(inside)
                                        .on(zgui_view::events::KEY_DOWN, move |_| {
                                            heard.set(heard.get() + 1)
                                        }),
                                ),
                            )
                        } else {
                            AnyView::new(())
                        }
                    }))
                    .into_view()
                    .build(cx),
            )
        },
    );
    app.settle(8);
    let before = draws(&app);
    let configured = app.app().windows()[0].surface_configures();
    app.deliver_to_first(SurfaceEvent::Pointer {
        action: PointerAction::Pressed,
        event: PointerEvent::mouse(Point::new(CssPx(12.0), CssPx(12.0))),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    settle(&mut app);
    assert!(
        inside.get().is_some(),
        "the pointer's reactive child must be mounted"
    );
    assert_eq!(
        app.app().windows()[0].host().focused().get_untracked(),
        trigger.get()
    );

    // The host can consume the click's command, install the modal trap and obtain its final
    // focus before deciding whether the following key belongs to the UI or embedded content.
    let _trap = dialog
        .trap_focus(FocusTrapOptions::MODAL)
        .expect("live dialog");
    settle(&mut app);
    assert_eq!(
        app.app().windows()[0].host().focused().get_untracked(),
        inside.get()
    );
    app.deliver_to_first(SurfaceEvent::Key {
        state: KeyState::Pressed,
        event: KeyEvent::named(NamedKey::Enter, PhysicalKey::Code(KeyCode::Enter)),
        modifiers: Modifiers::NONE,
        timestamp: Timestamp::ORIGIN,
    });
    settle(&mut app);
    assert_eq!(keys.get(), 1);
    assert_eq!(draws(&app), before, "settlement entered the renderer");
    assert_eq!(app.app().windows()[0].surface_configures(), configured);

    app.settle(8);
    assert!(draws(&app) > before, "the pending redraw was lost");
    let after = draws(&app);
    settle(&mut app);
    assert_eq!(draws(&app), after);
    app.shut_down();
}
