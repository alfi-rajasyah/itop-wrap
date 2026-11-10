import ITopKit
import SwiftUI

/// Where New ticket reads its lists and creates: iTop in the app, memory in the debug demo.
struct NewTicketSource: Sendable {
    var create: @Sendable (NewTicket) async throws -> Ticket
    var organizations: @Sendable () async throws -> [ServiceRef]
    var callers: @Sendable (String) async throws -> [ServiceRef]
    var services: @Sendable (String) async throws -> [ServiceRef]
    var subcategories: @Sendable (String) async throws -> [ServiceRef]
    var canCreate: Bool

    static func live(_ session: Session) -> NewTicketSource {
        let writes = TicketWrites(client: session.client)
        let lists = PickLists(client: session.client)
        let catalog = ServiceCatalog(client: session.client)
        return NewTicketSource(create: { try await writes.create($0) },
                               organizations: { try await lists.organizations() },
                               callers: { try await lists.callers(inOrg: $0) },
                               services: { try await catalog.services(forOrg: $0) },
                               subcategories: { try await catalog.subcategories(ofService: $0) },
                               canCreate: session.client.canCreateTickets)
    }
}

/// How New ticket was opened: plain, from an organization, or from a config item.
struct NewTicketStart: Identifiable, Equatable {
    let id = UUID()
    var orgID: String?
    var orgName: String?
    var ciID: String?
    var ciName: String?
}

/// New ticket (PLAN.md Task 9; UX-FLOWS F7): organization first, then caller, service and
/// subcategory, impact and urgency, title and description. Cancel keeps the draft.
struct NewTicketSheet: View {
    let workspace: Workspace
    let start: NewTicketStart
    let opened: (Ticket) -> Void
    @Environment(\.dismiss) private var dismiss

    @State private var draft = NewTicket(ticketClass: .userRequest)
    @State private var orgs: Loadable<[ServiceRef]> = .loading
    @State private var callers: Loadable<[ServiceRef]> = .idle
    @State private var services: Loadable<[ServiceRef]> = .idle
    @State private var subcategories: Loadable<[ServiceRef]> = .idle
    @State private var org: ServiceRef?
    @State private var caller: ServiceRef?
    @State private var service: ServiceRef?
    @State private var subcategory: ServiceRef?
    @State private var impact: ServiceRef? = PickLists.impacts.first
    @State private var urgency: ServiceRef? = PickLists.urgencies.last
    @State private var tried = false
    @State private var creating = false
    @State private var error: String?
    /// Set while restoring a draft or a prefill, so the cascade doesn't clear the children.
    @State private var restoring = true

    private var source: NewTicketSource { workspace.newTickets }
    private var titleEmpty: Bool { draft.title.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
    private var descriptionEmpty: Bool { draft.description.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    var body: some View {
        SheetFrame(title: "New ticket", subtitle: start.ciName.map { "On \($0)" }) {
            Picker("Type", selection: $draft.ticketClass) {
                Text("User request").tag(TicketClass.userRequest)
                Text("Incident").tag(TicketClass.incident)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .padding(.bottom, 2)

            RequiredLabel("Organization", missing: tried && org == nil)
            DropdownField(name: "Organization", placeholder: placeholder(orgs, loading: "Loading organizations…", empty: "No organizations found"),
                          items: orgs.value ?? [], selection: $org, label: \.name,
                          searchPrompt: "Search organizations", noMatch: "No organization matches")
                .disabled(orgs.value == nil)
            RequiredLabel("Caller", missing: tried && caller == nil)
            DropdownField(name: "Caller", placeholder: org == nil ? "Choose an organization first" : placeholder(callers, loading: "Loading people…", empty: "Nobody in \(org?.name ?? "this organization")"),
                          items: org == nil ? [] : (callers.value ?? []), selection: $caller, label: \.name,
                          searchPrompt: "Search people in \(org?.name ?? "the organization")",
                          noMatch: "No caller in \(org?.name ?? "this organization") matches")
                .disabled(org == nil || callers.value == nil)
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    RequiredLabel("Service", missing: tried && service == nil)
                    DropdownField(name: "Service", placeholder: org == nil ? "Choose an organization first" : placeholder(services, loading: "Loading…", empty: "No services"),
                                  items: org == nil ? [] : (services.value ?? []), selection: $service, label: \.name)
                        .disabled(org == nil || services.value == nil)
                }
                VStack(alignment: .leading, spacing: 4) {
                    RequiredLabel("Subcategory", missing: tried && subcategory == nil)
                    DropdownField(name: "Service subcategory", placeholder: service == nil ? "Choose a service first" : placeholder(subcategories, loading: "Loading…", empty: "None for this service"),
                                  items: service == nil ? [] : (subcategories.value ?? []), selection: $subcategory, label: \.name)
                        .disabled(service == nil || subcategories.value == nil)
                }
            }
            HStack(alignment: .top, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text("Impact").font(.system(size: 12, weight: .medium)).foregroundStyle(Theme.secondary).padding(.top, 4)
                    DropdownField(name: "Impact", placeholder: "Choose…", items: PickLists.impacts, selection: $impact, label: \.name)
                }
                VStack(alignment: .leading, spacing: 4) {
                    Text("Urgency").font(.system(size: 12, weight: .medium)).foregroundStyle(Theme.secondary).padding(.top, 4)
                    DropdownField(name: "Urgency", placeholder: "Choose…", items: PickLists.urgencies, selection: $urgency, label: \.name)
                }
            }
            RequiredLabel("Title", missing: tried && titleEmpty)
            TextField("What's wrong, in a few words", text: $draft.title)
                .textFieldStyle(.roundedBorder)
                .accessibilityLabel("Title, required")
            RequiredLabel("Description", missing: tried && descriptionEmpty)
            NoteEditor(text: $draft.description, placeholder: "What the caller reports, and anything you've already checked.", height: 100)
                .accessibilityLabel("Description, required")
            if tried, let first = firstMissing { FieldError("Fill in the \(first) first.") }
            if let error { FieldError(error) }
            if !source.canCreate {
                Text("Creating tickets is switched off in this build, so nothing is sent to iTop.")
                    .font(.system(size: 12)).foregroundStyle(Theme.secondary)
            }
        } buttons: {
            if workspace.newTicketDraft != nil {
                Button("Clear form") { clear() }
                Spacer()
            }
            Button("Cancel") {
                keepDraft()
                dismiss()
            }
            .cancelShortcutUnlessDropdownOpen()
            ConfirmButton(title: "Create ticket", busy: creating) { Task { await create() } }
                .disabled(creating || !source.canCreate)
        }
        .frame(width: 480)
        .task { await load() }
        .onChange(of: org) { _, new in
            if !restoring { caller = nil; service = nil; subcategory = nil }
            callers = .idle; services = .idle
            if let new { Task { await loadForOrg(new) } }
        }
        .onChange(of: service) { _, new in
            if !restoring { subcategory = nil }
            subcategories = .idle
            if let new { Task { await loadSubcategories(new) } }
        }
    }

    private var firstMissing: String? {
        if org == nil { return "organization" }
        if caller == nil { return "caller" }
        if service == nil { return "service" }
        if subcategory == nil { return "subcategory" }
        if titleEmpty { return "title" }
        if descriptionEmpty { return "description" }
        return nil
    }

    private func placeholder(_ state: Loadable<[ServiceRef]>, loading: String, empty: String) -> String {
        switch state {
        case .idle, .loading: return loading
        case .failed(let message): return "Couldn't load: \(message)"
        case .loaded(let items): return items.isEmpty ? empty : "Choose…"
        }
    }

    // MARK: Loading and the dependent lists

    private func load() async {
        // A kept draft wins over a plain start; a start from an org or CI begins fresh.
        if let kept = workspace.newTicketDraft, start.orgID == nil, start.ciID == nil {
            draft = kept
        } else {
            draft.ciID = start.ciID
        }
        do {
            let all = try await source.organizations()
            orgs = .loaded(all)
            let wanted = start.orgID ?? (draft.orgID.isEmpty ? nil : draft.orgID)
            org = all.first { $0.id == wanted } ?? start.orgName.flatMap { name in all.first { $0.name == name } }
            impact = PickLists.impacts.first { $0.id == draft.impact } ?? impact
            urgency = PickLists.urgencies.first { $0.id == draft.urgency } ?? urgency
            if org == nil { restoring = false }
        } catch {
            orgs = .failed(ErrorText.describe(error))
            restoring = false
        }
    }

    private func loadForOrg(_ chosen: ServiceRef) async {
        callers = .loading
        services = .loading
        async let people = source.callers(chosen.id)
        async let offered = source.services(chosen.id)
        do {
            let p = try await people
            guard org == chosen else { return }
            callers = .loaded(p)
            if restoring { caller = p.first { $0.id == draft.callerID } }
        } catch {
            callers = .failed(ErrorText.describe(error))
        }
        do {
            let s = try await offered
            guard org == chosen else { return }
            services = .loaded(s)
            if restoring {
                service = s.first { $0.id == draft.serviceID }
                if service == nil { restoring = false }
            }
        } catch {
            services = .failed(ErrorText.describe(error))
            restoring = false
        }
    }

    private func loadSubcategories(_ chosen: ServiceRef) async {
        subcategories = .loading
        do {
            let found = try await source.subcategories(chosen.id)
            guard service == chosen else { return }
            subcategories = .loaded(found)
            if restoring { subcategory = found.first { $0.id == draft.subcategoryID } }
        } catch {
            subcategories = .failed(ErrorText.describe(error))
        }
        restoring = false
    }

    // MARK: Draft and create

    private func filled() -> NewTicket {
        var d = draft
        d.orgID = org?.id ?? ""
        d.callerID = caller?.id ?? ""
        d.serviceID = service?.id ?? ""
        d.subcategoryID = subcategory?.id ?? ""
        d.impact = impact?.id ?? "1"
        d.urgency = urgency?.id ?? "4"
        return d
    }

    /// Cancel keeps what you typed (UX-FLOWS F7: an accidental close shouldn't lose it).
    private func keepDraft() {
        let d = filled()
        let empty = d.orgID.isEmpty && d.title.trimmingCharacters(in: .whitespaces).isEmpty && d.description.trimmingCharacters(in: .whitespaces).isEmpty
        workspace.newTicketDraft = empty ? nil : d
    }

    private func clear() {
        workspace.newTicketDraft = nil
        draft = NewTicket(ticketClass: draft.ticketClass)
        restoring = false
        org = nil; caller = nil; service = nil; subcategory = nil
        impact = PickLists.impacts.first; urgency = PickLists.urgencies.last
        tried = false; error = nil
    }

    private func create() async {
        tried = true
        error = nil
        guard firstMissing == nil else { return }
        creating = true
        defer { creating = false }
        UITrace.log("new ticket: creating a \(draft.ticketClass.rawValue)")
        do {
            let ticket = try await source.create(filled())
            workspace.newTicketDraft = nil
            dismiss()
            opened(ticket)
        } catch let failure {
            error = TicketListModel.actionError(failure, ticket: Ticket(key: "0", ticketClass: draft.ticketClass, ref: "the ticket", title: "",
                                                                      status: "", priority: 0, callerName: "", orgName: "", teamID: "0",
                                                                      teamName: "", agentID: "0", agentName: "", startDate: nil, lastUpdate: nil),
                                                verb: "create the ticket")
        }
    }
}
