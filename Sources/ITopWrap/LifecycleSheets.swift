import ITopKit
import SwiftUI

/// Set to pending (DESIGN.md: Sheets). The reason is required and saved to `pending_reason`.
/// Escape cancels, ⌘↩ confirms.
struct PendingSheet: View {
    let ticket: Ticket
    let list: TicketListModel
    @Environment(\.dismiss) private var dismiss
    @State private var reason = ""
    @State private var error: String?
    @State private var tried = false

    private var empty: Bool { reason.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    var body: some View {
        SheetFrame(title: "Set \(ticket.ref) to pending", subtitle: ticket.title) {
            RequiredLabel("Reason", missing: tried && empty)
            NoteEditor(text: $reason, placeholder: "What are you waiting for? For example the caller's reply or a part.", height: 90, autofocus: true)
                .accessibilityLabel("Reason, required")
            if tried && empty { FieldError("Enter a reason.") }
            if let error { FieldError(error) }
        } buttons: {
            Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            ConfirmButton(title: "Set to pending", busy: list.working == ticket.id) {
                tried = true
                guard !empty else { return }
                Task { if let message = await list.apply(.pending(reason: reason), to: ticket) { error = message } else { dismiss() } }
            }
            .disabled(list.working != nil)
        }
    }
}

/// Resolve (DESIGN.md: Resolve sheet): service and subcategory prefilled from the ticket and
/// required, a resolution code, and a required solution. No time field.
struct ResolveSheet: View {
    let ticket: Ticket
    let list: TicketListModel
    let detail: TicketDetail?
    @Environment(\.dismiss) private var dismiss

    @State private var services: Loadable<[ServiceRef]> = .loading
    @State private var subcategories: Loadable<[ServiceRef]> = .idle
    @State private var service: ServiceRef?
    @State private var subcategory: ServiceRef?
    @State private var code: ResolutionCode? = ResolutionCode.standard.first
    @State private var solution = ""
    @State private var error: String?
    @State private var tried = false
    /// The first service change is the prefill; it keeps the ticket's subcategory.
    @State private var prefilling = true

    private var solutionEmpty: Bool { solution.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }

    var body: some View {
        SheetFrame(title: "Resolve \(ticket.ref)", subtitle: ticket.title) {
            RequiredLabel("Service", missing: tried && service == nil)
            DropdownField(placeholder: placeholder(services, loading: "Loading services…", empty: "No services found"),
                          items: services.value ?? [], selection: $service, label: \.name)
                .disabled(services.value == nil)
            RequiredLabel("Service subcategory", missing: tried && subcategory == nil)
            DropdownField(placeholder: service == nil ? "Choose a service first"
                                                      : placeholder(subcategories, loading: "Loading subcategories…", empty: "This service has no subcategories"),
                          items: service == nil ? [] : (subcategories.value ?? []), selection: $subcategory, label: \.name)
                .disabled(service == nil || subcategories.value == nil)
            Text("Resolution code").font(.system(size: 12, weight: .medium)).foregroundStyle(Theme.secondary)
            DropdownField(placeholder: "Choose…", items: ResolutionCode.standard, selection: $code, label: \.name)
            RequiredLabel("Solution", missing: tried && solutionEmpty)
            NoteEditor(text: $solution, placeholder: "What fixed it, in words the caller understands.", height: 110, autofocus: true)
                .accessibilityLabel("Solution, required")
            if tried && solutionEmpty { FieldError("Enter a solution.") }
            if let error { FieldError(error) }
        } buttons: {
            Button("Cancel") { dismiss() }.keyboardShortcut(.cancelAction)
            ConfirmButton(title: "Resolve ticket", busy: list.working == ticket.id) {
                tried = true
                guard let service, let subcategory, let code, !solutionEmpty else { return }
                let r = Resolution(serviceID: service.id, subcategoryID: subcategory.id, code: code.id, solution: solution)
                Task { if let message = await list.apply(.resolve(r), to: ticket) { error = message } else { dismiss() } }
            }
            .disabled(list.working != nil)
        }
        .task { await loadServices() }
        .onChange(of: service) { _, new in
            if prefilling { prefilling = false } else { subcategory = nil }
            subcategories = .idle
            if let new { Task { await loadSubcategories(new) } }
        }
    }

    private func placeholder(_ state: Loadable<[ServiceRef]>, loading: String, empty: String) -> String {
        switch state {
        case .idle, .loading: return loading
        case .failed(let message): return "Couldn't load: \(message)"
        case .loaded(let items): return items.isEmpty ? empty : "Choose…"
        }
    }

    private func loadServices() async {
        do {
            var all = try await list.source.services(ticket)
            // Prefill from the ticket, even if its service isn't in the offered list. Opened
            // before the detail finished loading, read it here.
            var loaded = detail
            if loaded == nil { loaded = try? await list.source.detail(ticket) }
            if let d = loaded, TicketWrites.isID(d.serviceID), d.serviceID != "0" {
                let current = ServiceRef(id: d.serviceID, name: d.serviceName.isEmpty ? "Service \(d.serviceID)" : d.serviceName)
                if !all.contains(where: { $0.id == current.id }) { all.insert(current, at: 0) }
                if TicketWrites.isID(d.subcategoryID), d.subcategoryID != "0" {
                    subcategory = ServiceRef(id: d.subcategoryID, name: d.subcategoryName.isEmpty ? "Subcategory \(d.subcategoryID)" : d.subcategoryName)
                }
                services = .loaded(all)
                service = all.first { $0.id == current.id }
            } else {
                prefilling = false
                services = .loaded(all)
            }
        } catch {
            services = .failed(ErrorText.describe(error))
        }
    }

    private func loadSubcategories(_ chosen: ServiceRef) async {
        subcategories = .loading
        do {
            var found = try await list.source.subcategories(chosen.id)
            guard service == chosen else { return }
            if let s = subcategory, !found.contains(s) { found.insert(s, at: 0) }
            subcategories = .loaded(found)
        } catch {
            subcategories = .failed(ErrorText.describe(error))
        }
    }
}

// MARK: - Shared sheet pieces

/// Title, subtitle, a single column of fields, and the buttons at the bottom right.
struct SheetFrame<Fields: View, Buttons: View>: View {
    let title: String
    let subtitle: String
    @ViewBuilder let fields: Fields
    @ViewBuilder let buttons: Buttons

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).font(.system(size: 15, weight: .semibold))
                Text(subtitle).foregroundStyle(Theme.secondary).lineLimit(2)
            }
            .padding(.bottom, 6)
            fields
            HStack {
                Spacer()
                buttons
            }
            .padding(.top, 8)
        }
        .padding(20)
        .frame(width: 420)
    }
}

/// A field label that says "Required" from the start (DESIGN.md: Forms), red once it's missing.
struct RequiredLabel: View {
    let title: String
    let missing: Bool

    init(_ title: String, missing: Bool) {
        self.title = title
        self.missing = missing
    }

    var body: some View {
        HStack(spacing: 6) {
            Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(Theme.secondary)
            Text("Required").font(.system(size: 11)).foregroundStyle(missing ? Theme.warning : Theme.secondary)
        }
        .padding(.top, 4)
    }
}

struct FieldError: View {
    let text: String
    init(_ text: String) { self.text = text }

    var body: some View {
        Text(text).font(.system(size: 12)).foregroundStyle(Theme.warning).fixedSize(horizontal: false, vertical: true)
    }
}

/// A multi-line text field with a placeholder, styled like the log composer.
struct NoteEditor: View {
    @Binding var text: String
    let placeholder: String
    let height: CGFloat
    var autofocus = false
    @FocusState private var focused: Bool

    var body: some View {
        ZStack(alignment: .topLeading) {
            TextEditor(text: $text)
                .font(.system(size: 13))
                .scrollContentBackground(.hidden)
                .focused($focused)
            if text.isEmpty {
                Text(placeholder).foregroundStyle(Theme.secondary).padding(.leading, 5).padding(.top, 1).allowsHitTesting(false)
            }
        }
        .padding(8)
        .frame(height: height)
        .background(Theme.chip.opacity(0.35), in: RoundedRectangle(cornerRadius: 8))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(focused ? Theme.text.opacity(0.5) : Theme.separator))
        .onAppear { if autofocus { focused = true } }
    }
}

/// The confirming button of a sheet, named for its action; ⌘↩ presses it.
struct ConfirmButton: View {
    let title: String
    let busy: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: 6) {
                if busy { ProgressView().controlSize(.mini) }
                Text(title)
            }
        }
        .buttonStyle(.borderedProminent)
        .tint(Theme.primaryFill)
        .foregroundStyle(Theme.onPrimary)
        .keyboardShortcut(.return, modifiers: .command)
    }
}
