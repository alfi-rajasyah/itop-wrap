import AppKit
import ITopKit
import SwiftUI

/// The middle column for Config items: search results (DESIGN.md: CMDB).
struct CMDBListView: View {
    @Bindable var model: CMDBModel

    var body: some View {
        VStack(spacing: 0) {
            if let scope = model.scope {
                HStack {
                    Label("Config items in \(scope.name)", systemImage: "building.2").font(.system(size: 12))
                    Spacer()
                    Button { model.clearScope() } label: { Image(systemName: "xmark.circle.fill") }
                        .buttonStyle(.plain).foregroundStyle(Theme.secondary)
                        .help("Show search instead").accessibilityLabel("Clear organization")
                }
                .padding(.horizontal, 14).padding(.vertical, 8)
                .background(.bar)
            }
            content
        }
        .searchable(text: $model.query, placement: .toolbar, prompt: "Search config items")
        .task(id: model.query + (model.scope?.id ?? "")) { await model.search() }
    }

    @ViewBuilder
    private var content: some View {
        switch model.results {
        case .idle:
            message("Search by name, for example a server or application.")
        case .searching:
            VStack(spacing: 10) {
                ProgressView().controlSize(.small)
                Text("Searching iTop").foregroundStyle(Theme.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .failed(let error):
            message("Couldn't search iTop. \(error)", warning: true)
        case .loaded(let items) where items.isEmpty:
            message("No config items match “\(model.query.trimmingCharacters(in: .whitespaces))”.")
        case .loaded(let items):
            List(selection: $model.selectedID.deferred("config-items")) {
                Section {
                    ForEach(items) { item in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(item.name).font(.system(size: 12.5, design: .monospaced)).lineLimit(1)
                            Text([item.classLabel, item.orgName].filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.system(size: 11)).foregroundStyle(Theme.secondary).lineLimit(1)
                        }
                        .padding(.vertical, 3)
                        .tag(item.id)
                    }
                } header: {
                    Text(items.count == 1 ? "1 result" : items.count >= CMDBQueries.searchLimit ? "First \(items.count) results" : "\(items.count) results")
                        .font(.system(size: 11, weight: .semibold)).foregroundStyle(Theme.secondary)
                }
            }
            .listStyle(.inset)
        }
    }

    private func message(_ text: String, warning: Bool = false) -> some View {
        Text(text)
            .foregroundStyle(warning ? Theme.warning : Theme.secondary)
            .multilineTextAlignment(.center)
            .padding(24)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

/// The CI on the right: fields, relationships, open tickets (DESIGN.md: Config item detail).
struct CIDetailView: View {
    let model: CMDBModel
    let item: ConfigItem
    var openTicket: (Ticket) -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                header
                switch model.detail {
                case .loading:
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
                        Text("Loading \(item.name) from iTop").foregroundStyle(Theme.secondary)
                    }
                case .failed(let error):
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Couldn't load this config item. \(error)").foregroundStyle(Theme.warning)
                        Button("Retry") { Task { await model.loadDetail() } }
                    }
                case .loaded(let d):
                    fields(d)
                    if !d.description.isEmpty {
                        VStack(alignment: .leading, spacing: 4) {
                            Text("Description").font(.system(size: 12, weight: .semibold))
                            Text(d.description).fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
                        }
                    }
                    relationships(d)
                    tickets(d)
                }
            }
            .padding(.horizontal, 24).padding(.vertical, 18)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .font(.system(size: 13))
        .task(id: item.id) { await model.loadDetail() }
    }

    private var status: String? {
        if case .loaded(let d) = model.detail { return d.fields.first { $0.label == "Status" }?.value }
        return nil
    }

    private var header: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                Text([item.classLabel, status].compactMap { $0 }.joined(separator: " · "))
                    .font(.system(size: 12)).foregroundStyle(Theme.secondary)
                Text(item.name)
                    .font(.system(size: 22, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
                if model.copied {
                    Label("Copied \(item.name)", systemImage: "checkmark")
                        .font(.system(size: 12))
                        .padding(.horizontal, 8).padding(.vertical, 2)
                        .background(Theme.chip, in: RoundedRectangle(cornerRadius: 5))
                }
            }
            Spacer(minLength: 0)
            Button {
                Task { await model.copyName { name in
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(name, forType: .string)
                } }
            } label: { Image(systemName: "doc.on.doc") }
            .help("Copy name").accessibilityLabel("Copy name")
            if let url = model.webURL(item) {
                Link(destination: url) { Image(systemName: "arrow.up.forward.square") }
                    .help("Open in iTop on the web").accessibilityLabel("Open in iTop on the web")
                    .foregroundStyle(Theme.text)
            }
        }
    }

    private func fields(_ d: ConfigItemDetail) -> some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 160), alignment: .topLeading)], alignment: .leading, spacing: 12) {
            ForEach(d.fields) { f in
                VStack(alignment: .leading, spacing: 2) {
                    Text(f.label).font(.system(size: 11)).foregroundStyle(Theme.secondary)
                    Text(f.value).textSelection(.enabled)
                }
            }
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Theme.chip.opacity(0.6), in: RoundedRectangle(cornerRadius: 8))
    }

    private func relationships(_ d: ConfigItemDetail) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Relationships").font(.system(size: 14, weight: .semibold))
            HStack(alignment: .top, spacing: 24) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Depends on").font(.system(size: 12)).foregroundStyle(Theme.secondary)
                    if d.dependsOn.isEmpty { Text("Nothing listed in the CMDB.").foregroundStyle(Theme.secondary) }
                    chips(d.dependsOn)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                VStack(alignment: .leading, spacing: 6) {
                    Text("Used by").font(.system(size: 12)).foregroundStyle(Theme.secondary)
                    chips(d.usedBy)
                    Text(d.usedBy.isEmpty ? "Nothing in the CMDB depends on \(item.name)."
                                          : "If \(item.name) fails, \(ListFormatter.localizedString(byJoining: d.usedBy.map(\.name))) \(d.usedBy.count == 1 ? "is" : "are") affected.")
                        .font(.system(size: 12)).foregroundStyle(Theme.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func chips(_ refs: [CIRef]) -> some View {
        FlowLayout(spacing: 6, lineSpacing: 6) {
            ForEach(refs) { ref in
                Button { model.open(ref.item) } label: {
                    Text(ref.name).font(.system(size: 12, design: .monospaced))
                        .padding(.horizontal, 10).padding(.vertical, 5)
                        .overlay(RoundedRectangle(cornerRadius: 6).stroke(Theme.separator))
                        .contentShape(RoundedRectangle(cornerRadius: 6))
                }
                .buttonStyle(.plain)
                .help("\(ConfigItem(key: ref.id, className: ref.className, name: ref.name, orgName: "").classLabel): open \(ref.name)")
            }
        }
    }

    private func tickets(_ d: ConfigItemDetail) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                Text("Open tickets on this CI").font(.system(size: 14, weight: .semibold))
                Text("\(d.openTickets.count)").foregroundStyle(Theme.secondary)
            }
            if d.openTickets.isEmpty { Text("No open tickets on \(item.name).").foregroundStyle(Theme.secondary) }
            ForEach(d.openTickets) { t in
                Button { openTicket(t) } label: {
                    HStack(spacing: 10) {
                        if TicketList.needsYou(t, me: model.me) {
                            Circle().fill(Theme.accent).frame(width: 8, height: 8).accessibilityLabel("Needs you")
                        }
                        Text(t.ref).font(.system(size: 11.5, design: .monospaced)).foregroundStyle(Theme.secondary)
                        Text(t.title).fontWeight(.medium).lineLimit(1)
                        Spacer(minLength: 8)
                        Text("\(TicketGroup.statusLabel(t.status)) · \(t.isUnassigned ? "Unassigned" : (t.agentID == model.me.contactID ? "You" : t.agentName))")
                            .font(.system(size: 12)).foregroundStyle(Theme.secondary).lineLimit(1)
                    }
                    .padding(.horizontal, 12).padding(.vertical, 10)
                    .overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.separator))
                    .contentShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
                .help("Open \(t.ref)")
            }
        }
    }
}
