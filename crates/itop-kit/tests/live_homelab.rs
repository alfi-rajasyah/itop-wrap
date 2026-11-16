//! A run against a real iTop, off by default. It reads and WRITES (creates a ticket and walks it
//! to closed), so point it only at a test server:
//!
//!   ITOP_URL=… ITOP_USER=… ITOP_PASSWORD=… cargo test -p itop-kit --test live_homelab -- --ignored --nocapture
//!
//! It prints what it did, never the password.

use std::sync::Arc;

use itop_kit::*;

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

/// A 2×2 red PNG.
fn tiny_png() -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEElEQVR4nGP4z8DAwMDAAAAGAQEAvqCkswAAAABJRU5ErkJggg==")
        .unwrap()
}

#[tokio::test]
#[ignore = "needs a real iTop test server; it writes"]
async fn the_whole_round_against_a_real_itop() {
    let server = ServerAddress::new(&env("ITOP_URL")).unwrap();
    let user = env("ITOP_USER");
    let transport: Arc<dyn Transport> = Arc::new(ReqwestTransport::new().unwrap());
    let client = ITopClient::with_writes(
        server,
        Credentials::new(user.clone(), env("ITOP_PASSWORD")),
        transport,
        true,
        None,
        true,
    );

    assert!(client.check_credentials().await.unwrap(), "sign-in");
    let me = CurrentUser::load(&user, &client).await.unwrap();
    println!(
        "signed in as {} (contact {}), teams: {:?}",
        me.display_name,
        me.contact_id,
        me.teams.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    let tickets = TicketService::new(client.clone());
    let mine = tickets.load_mine(&me, &TicketScope::OPEN).await.unwrap();
    let team = tickets.load_team(&me, &TicketScope::OPEN).await.unwrap();
    println!("my tickets: {}, team tickets: {}", mine.len(), team.len());

    let orgs = OrgService::new(client.clone());
    let all_orgs = orgs.organizations().await.unwrap();
    println!("organizations: {}", all_orgs.len());
    let people = orgs.people_page_matching("an").await.unwrap();
    println!(
        "people matching 'an': {} of {}",
        people.people.len(),
        people.total
    );
    let cmdb = CMDBService::new(client.clone());
    let cis = cmdb.search("se").await.unwrap();
    println!("config items matching 'se': {}", cis.len());
    if let Some(ci) = cis.first() {
        let d = cmdb.detail(ci).await.unwrap();
        println!(
            "  {}: {} fields, {} depends on, {} used by, {} open tickets",
            ci.name,
            d.fields.len(),
            d.depends_on.len(),
            d.used_by.len(),
            d.open_tickets.len()
        );
    }

    // New ticket: the first org that has a caller and a service with a subcategory.
    let picks = PickLists::new(client.clone());
    let catalog = ServiceCatalog::new(client.clone());
    let mut draft = None;
    for org in picks.organizations().await.unwrap() {
        let callers = picks.callers(&org.id).await.unwrap();
        let services = catalog.services_for_org(&org.id).await.unwrap();
        for service in &services {
            let subs = catalog.subcategories(&service.id).await.unwrap();
            if let (Some(caller), Some(sub)) = (callers.first(), subs.first()) {
                let mut t = NewTicket::new(TicketClass::UserRequest);
                t.org_id = org.id.clone();
                t.caller_id = caller.id.clone();
                t.service_id = service.id.clone();
                t.subcategory_id = sub.id.clone();
                t.title = "iTop Wrap Rust port: live test".into();
                t.description = "Created by the itop-kit live test.\nSecond line with <b>markup</b> & an ampersand.".into();
                println!(
                    "new ticket for org {}, caller {}, service {}, subcategory {}",
                    org.name, caller.name, service.name, sub.name
                );
                draft = Some(t);
                break;
            }
        }
        if draft.is_some() {
            break;
        }
    }
    let draft = draft.expect("no org with a caller, a service and a subcategory on this server");
    let writes = TicketWrites::new(client.clone());
    let ticket = writes.create(&draft).await.unwrap();
    println!("created {} ({})", ticket.r#ref, ticket.status);

    let found = tickets
        .lookup(&ticket.r#ref)
        .await
        .unwrap()
        .expect("lookup finds the new ticket");
    assert_eq!(found.key, ticket.key);

    writes
        .add_log_entry(&found, "Public entry from the live test.", false)
        .await
        .unwrap();
    writes
        .add_log_entry(&found, "Private entry from the live test.", true)
        .await
        .unwrap();
    let image = LogImage::new(tiny_png(), "red.png").unwrap();
    writes
        .add_log_entry_with_images(&found, "Entry with an image.", &[image], false)
        .await
        .unwrap();

    let detail = tickets.detail(&found).await.unwrap();
    println!(
        "detail: {} public, {} private entries; description {:?}",
        detail.public_log.len(),
        detail.private_log.len(),
        detail.description
    );
    assert_eq!(detail.public_log.len(), 2);
    assert_eq!(detail.private_log.len(), 1);
    let base = client.server.base_url().to_owned();
    let with_image = detail
        .public_log
        .iter()
        .find(|e| e.text.contains("Entry with an image"))
        .expect("image entry");
    let blocks = RichText::parse(with_image.html.as_deref().unwrap_or(""), &base);
    let inline = blocks.iter().find_map(|b| match b {
        RichBlock::Image(RichImage::Inline { id }) => Some(id.clone()),
        _ => None,
    });
    let id = inline.expect("the entry's image is an inline image");
    let loaded = InlineImageLoader::new(client.clone())
        .image(&id)
        .await
        .unwrap();
    println!(
        "inline image {id}: {} bytes, {}",
        loaded.data.len(),
        loaded.mimetype
    );
    assert_eq!(loaded.data, tiny_png());

    // Assign: to me when I am in a team, else to the first team's first agent.
    let mut current = found;
    println!(
        "actions when {}: {:?}",
        current.status,
        TicketLifecycle::actions(&current.status)
    );
    match writes.assign_to_me(&current, &me).await {
        Ok(()) => println!("assigned to me"),
        Err(e) => {
            println!("assign to me: {e}; assigning to a team's first agent instead");
            let dir = TeamDirectory::new(client.clone());
            let mut done = false;
            for t in dir.teams().await.unwrap() {
                if let Some(a) = dir.agents(&t.id).await.unwrap().first() {
                    writes.assign(&current, &t.id, &a.id).await.unwrap();
                    println!("assigned to {} in {}", a.name, t.name);
                    done = true;
                    break;
                }
            }
            assert!(done, "no team with an agent on this server");
        }
    }
    current = tickets.lookup(&current.r#ref).await.unwrap().unwrap();
    assert_eq!(current.status, "assigned");

    let services = catalog.services(&current).await.unwrap();
    let service = services
        .iter()
        .find(|s| s.id == draft.service_id)
        .unwrap_or(&services[0]);
    let resolution = Resolution::new(
        service.id.clone(),
        draft.subcategory_id.clone(),
        "other",
        "Solved by the live test.",
    );
    writes
        .apply(&TicketChange::Resolve(resolution), &current)
        .await
        .unwrap();
    current = tickets.lookup(&current.r#ref).await.unwrap().unwrap();
    assert_eq!(current.status, "resolved");

    writes.apply(&TicketChange::Reopen, &current).await.unwrap();
    current = tickets.lookup(&current.r#ref).await.unwrap().unwrap();
    println!("reopened: {}", current.status);
    let resolution = Resolution::new(
        service.id.clone(),
        draft.subcategory_id.clone(),
        "other",
        "Solved again.",
    );
    writes
        .apply(&TicketChange::Resolve(resolution), &current)
        .await
        .unwrap();
    current = tickets.lookup(&current.r#ref).await.unwrap().unwrap();
    writes.apply(&TicketChange::Close, &current).await.unwrap();
    current = tickets.lookup(&current.r#ref).await.unwrap().unwrap();
    assert_eq!(current.status, "closed");
    println!("{} is closed", current.r#ref);

    // Pending last, on a second ticket. The app offers Reassign and Resolve on a pending ticket,
    // as the Swift app does, and iTop has the last word: a stock iTop (this test server) refuses
    // both ("Invalid stimulus"), so the ticket is left pending.
    let second = writes.create(&draft).await.unwrap();
    let dir = TeamDirectory::new(client.clone());
    let team = dir
        .teams()
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("a team");
    let agent = dir
        .agents(&team.id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("an agent");
    writes.assign(&second, &team.id, &agent.id).await.unwrap();
    let second = tickets.lookup(&second.r#ref).await.unwrap().unwrap();
    writes
        .apply(
            &TicketChange::Pending {
                reason: "Waiting, says the live test.".into(),
            },
            &second,
        )
        .await
        .unwrap();
    let second = tickets.lookup(&second.r#ref).await.unwrap().unwrap();
    assert_eq!(second.status, "pending");
    println!("{} is pending", second.r#ref);

    let time = TimeTracking::new(client.clone());
    match time.totals(std::slice::from_ref(&current)).await {
        Ok(t) => println!("time totals: {t:?}"),
        Err(e) => println!("time totals (no Time Tracking extension expected): {e}"),
    }
}

/// Edit ticket against a real iTop: plain edits read back, what an assigned ticket allows, and a
/// formatted description that must survive a save untouched. Leaves one ticket behind; its ref
/// is printed so it can be removed.
#[tokio::test]
#[ignore = "needs a real iTop test server; it writes"]
async fn editing_a_ticket_against_a_real_itop() {
    let server = ServerAddress::new(&env("ITOP_URL")).unwrap();
    let user = env("ITOP_USER");
    let transport: Arc<dyn Transport> = Arc::new(ReqwestTransport::new().unwrap());
    let client = ITopClient::with_writes(
        server,
        Credentials::new(user.clone(), env("ITOP_PASSWORD")),
        transport,
        true,
        None,
        true,
    );
    let me = CurrentUser::load(&user, &client).await.unwrap();
    let picks = PickLists::new(client.clone());
    let catalog = ServiceCatalog::new(client.clone());
    let mut found = None;
    'orgs: for org in picks.organizations().await.unwrap() {
        let callers = picks.callers(&org.id).await.unwrap();
        if callers.len() < 2 {
            continue;
        }
        for service in catalog.services_for_org(&org.id).await.unwrap() {
            let subs = catalog.subcategories(&service.id).await.unwrap();
            if let Some(sub) = subs.first() {
                found = Some((org, callers, service, sub.clone()));
                break 'orgs;
            }
        }
    }
    let (org, callers, service, sub) =
        found.expect("no org with two callers and a service with a subcategory");
    let mut draft = NewTicket::new(TicketClass::UserRequest);
    draft.org_id = org.id.clone();
    draft.caller_id = callers[0].id.clone();
    draft.service_id = service.id.clone();
    draft.subcategory_id = sub.id.clone();
    draft.title = "iTop Wrap edit: live test".into();
    draft.description = "Before the edit.".into();
    let writes = TicketWrites::new(client.clone());
    let ticket = writes.create(&draft).await.unwrap();
    println!(
        "created {} for {} ({})",
        ticket.r#ref, org.name, ticket.status
    );

    // 1. A plain edit of every simple field, in the ticket's first state.
    let read = writes.editable(&ticket).await.unwrap();
    assert_eq!(
        read.description_text.as_deref(),
        Some("Before the edit."),
        "plain"
    );
    let mut after = read.values.clone();
    after.title = "iTop Wrap edit: live test (edited)".into();
    after.urgency = "2".into();
    after.impact = "2".into();
    after.caller_id = callers[1].id.clone();
    after.description = "After the edit.\nSecond line & <angle>.".into();
    let sent = writes
        .save_edit(&ticket, &read.values, &after, false)
        .await
        .unwrap();
    println!("status {}: sent {sent:?}", ticket.status);
    let back = writes.editable(&ticket).await.unwrap();
    assert_eq!(back.values.title, after.title);
    assert_eq!(back.values.urgency, "2");
    assert_eq!(back.values.impact, "2");
    assert_eq!(back.values.caller_id, callers[1].id);
    assert_eq!(
        back.description_text.as_deref(),
        Some("After the edit.\nSecond line & <angle>.")
    );

    // 2. Assigned: what stock iTop still lets REST change.
    // Any team with an agent (the test account itself may be in none).
    let directory = TeamDirectory::new(client.clone());
    let mut pick = None;
    for team in directory.teams().await.unwrap() {
        if let Some(agent) = directory.agents(&team.id).await.unwrap().into_iter().next() {
            pick = Some((team, agent));
            break;
        }
    }
    let (team, agent) = pick.expect("a team with an agent");
    let _ = &me;
    let assigned = writes.assign(&ticket, &team.id, &agent.id).await;
    println!("assign: {:?}", assigned.as_ref().map(|_| "ok"));
    let tickets = TicketService::new(client.clone());
    let now = tickets
        .lookup(&ticket.r#ref)
        .await
        .unwrap()
        .expect("still there");
    let read = writes.editable(&now).await.unwrap();
    for (label, change) in [
        (
            "title",
            Box::new(|t: &mut NewTicket| t.title = "iTop Wrap edit: assigned".into())
                as Box<dyn Fn(&mut NewTicket)>,
        ),
        (
            "caller",
            Box::new(|t: &mut NewTicket| t.caller_id = callers[0].id.clone()),
        ),
        (
            "urgency",
            Box::new(|t: &mut NewTicket| t.urgency = "3".into()),
        ),
    ] {
        let before = writes.editable(&now).await.unwrap();
        let mut after = before.values.clone();
        change(&mut after);
        let result = writes
            .save_edit(&now, &before.values, &after, before.description_locked())
            .await;
        println!(
            "status {}: change {label}: {}",
            now.status,
            match &result {
                Ok(_) => "accepted".to_owned(),
                Err(e) => format!("refused: {e:?}"),
            }
        );
    }
    let _ = read;

    // 3. A formatted description is locked and survives a save of another field.
    let mut fields = json::JsonObject::new();
    fields.insert(
        "description".into(),
        "<p>Has <strong>bold</strong> text</p>".into(),
    );
    let r = client
        .send(RestOperation::Update {
            class_name: "UserRequest".into(),
            key: now.key.clone(),
            fields,
            comment: "iTop Wrap live test".into(),
        })
        .await
        .unwrap();
    let object = r.objects().first().cloned().expect("the updated ticket");
    assert_eq!(object.code, 0, "{}", object.message);
    let read = writes.editable(&now).await.unwrap();
    assert!(read.description_locked(), "formatted → locked");
    let mut after = read.values.clone();
    after.title = "iTop Wrap edit: formatted description kept".into();
    let sent = writes
        .save_edit(&now, &read.values, &after, true)
        .await
        .unwrap();
    assert_eq!(sent, ["title"]);
    let html = client
        .send(RestOperation::get(
            "UserRequest",
            format!("SELECT UserRequest WHERE id = {}", now.key),
            &["description"],
        ))
        .await
        .unwrap();
    let description = html.objects()[0].fields["description"]
        .string_value()
        .unwrap_or("")
        .to_owned();
    assert!(
        description.contains("<strong>bold</strong>"),
        "description untouched: {description}"
    );
    println!("done: {} (remove it from the test server)", ticket.r#ref);
}
