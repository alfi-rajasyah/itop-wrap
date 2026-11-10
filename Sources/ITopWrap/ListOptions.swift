import Foundation
import ITopKit
import Observation
import SwiftUI

/// The date range and sort shared by every ticket list, remembered between launches.
@MainActor @Observable
final class ListOptions {
    enum Range: Int, CaseIterable, Identifiable {
        case any = 0, week = 7, month = 30, quarter = 90

        var id: Int { rawValue }
        var title: String {
            switch self {
            case .any: "Any time"
            case .week: "Last 7 days"
            case .month: "Last 30 days"
            case .quarter: "Last 90 days"
            }
        }
    }

    private let defaults: UserDefaults

    var range: Range { didSet { defaults.set(range.rawValue, forKey: "list.range") } }
    var includeClosed: Bool { didSet { defaults.set(includeClosed, forKey: "list.includeClosed") } }
    var sort: TicketSort { didSet { defaults.set(sort.rawValue, forKey: "list.sort") } }
    /// Tickets per page; `nil` shows them all (the "Show" menu).
    var pageSize: Int? { didSet { defaults.set(pageSize ?? 0, forKey: "list.pageSize") } }
    static let pageSizes: [Int?] = [10, 20, 30, 50, 100, nil]

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        range = Range(rawValue: defaults.integer(forKey: "list.range")) ?? .any
        includeClosed = defaults.bool(forKey: "list.includeClosed")
        sort = TicketSort(rawValue: defaults.string(forKey: "list.sort") ?? "") ?? .priority
        let size = defaults.object(forKey: "list.pageSize") as? Int ?? 20
        pageSize = size == 0 ? nil : size
    }

    var scope: TicketScope {
        TicketScope(includeClosed: includeClosed, openedWithinDays: range == .any ? nil : range.rawValue)
    }

    static let defaultPageSize: Int? = 20

    var isDefault: Bool { scope == .open && sort == .priority && pageSize == Self.defaultPageSize }

    func reset() {
        range = .any
        includeClosed = false
        sort = .priority
        pageSize = Self.defaultPageSize
    }

    /// What differs from the default, e.g. "All tickets, any time · Last update, newest first · 50 per page".
    var summary: String? {
        var parts: [String] = []
        if scope != .open { parts.append(rangeLabel) }
        if sort != .priority { parts.append(sort.title) }
        if pageSize != Self.defaultPageSize { parts.append(pageSize.map { "\($0) per page" } ?? "All on one page") }
        return parts.isEmpty ? nil : parts.joined(separator: " · ")
    }

    /// "Open tickets", "Open, last 30 days", "All, last 30 days".
    var rangeLabel: String {
        switch (includeClosed, range) {
        case (false, .any): "Open tickets"
        case (false, let r): "Open, \(r.title.lowercased())"
        case (true, .any): "All tickets, any time"
        case (true, let r): "All, \(r.title.lowercased())"
        }
    }
}

/// One "View" menu above every ticket list: which tickets, sort and page size (DESIGN.md).
/// A quiet line under it says what differs from the default.
struct ListViewMenu: View {
    @Bindable var options: ListOptions

    var body: some View {
        Menu {
            Section("Show tickets opened") {
                Picker("Opened", selection: $options.range) {
                    ForEach(ListOptions.Range.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
                Toggle("Include resolved and closed", isOn: $options.includeClosed)
            }
            Section("Sort inside each status group") {
                Picker("Sort", selection: $options.sort) {
                    ForEach(TicketSort.allCases) { Text($0.title).tag($0) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
            }
            Section("Per page") {
                Picker("Per page", selection: $options.pageSize) {
                    ForEach(ListOptions.pageSizes, id: \.self) { Text($0.map { "\($0) per page" } ?? "All").tag($0) }
                }
                .pickerStyle(.inline)
                .labelsHidden()
            }
            if !options.isDefault {
                Divider()
                Button("Back to default view") { options.reset() }
            }
        } label: {
            FilterChip(text: "View", symbol: "line.3.horizontal.decrease", on: !options.isDefault)
        }
        .menuStyle(.button)
        .buttonStyle(.plain)
        .menuIndicator(.visible)
        .fixedSize()
        .help(options.summary ?? "Open tickets, sorted by priority, 20 per page")
        .accessibilityLabel("View options: \(options.summary ?? "default")")
    }
}

/// "All tickets, any time · 20 per page", or nothing when the view is the default.
struct ListViewSummary: View {
    let options: ListOptions

    var body: some View {
        if let summary = options.summary {
            Text(summary)
                .font(.system(size: 11))
                .foregroundStyle(Theme.secondary)
                .lineLimit(2)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

/// The neutral button look for the org filter and View menus: grey icon and border,
/// a dark border when it's changed from the default. Never the accent colour.
struct FilterChip: View {
    let text: String
    let symbol: String
    let on: Bool

    var body: some View {
        HStack(spacing: 5) {
            Image(systemName: symbol).foregroundStyle(Theme.secondary)
            Text(text).foregroundStyle(Theme.text).lineLimit(1)
        }
        .font(.system(size: 12))
        .padding(.horizontal, 9).frame(height: 24)
        .overlay(RoundedRectangle(cornerRadius: 6).stroke(on ? Theme.text : Theme.separator))
        .contentShape(RoundedRectangle(cornerRadius: 6))
    }
}

/// "‹  21–40 of 556  ›" under a ticket list.
struct Pager: View {
    let page: TicketPage
    @Binding var current: Int

    var body: some View {
        if page.pageCount > 1 {
            HStack(spacing: 10) {
                Button { current = page.page - 1 } label: { Image(systemName: "chevron.left") }
                    .disabled(page.page <= 1)
                    .help("Previous page").accessibilityLabel("Previous page")
                Text(page.label).monospacedDigit()
                Button { current = page.page + 1 } label: { Image(systemName: "chevron.right") }
                    .disabled(page.page >= page.pageCount)
                    .help("Next page").accessibilityLabel("Next page")
            }
            .buttonStyle(.borderless)
            .font(.system(size: 12))
            .foregroundStyle(Theme.secondary)
        } else if page.total > 0 {
            Text(page.total == 1 ? "1 ticket" : "\(page.total) tickets").font(.system(size: 12)).foregroundStyle(Theme.secondary)
        }
    }
}
