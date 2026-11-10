import ITopKit
import SwiftUI

/// "Sign in to iTop" (DESIGN.md, Sign in; wireframe SignIn.dc.html).
struct LoginView: View {
    @Environment(SessionModel.self) private var model
    var notice: String?

    @State private var password = ""
    @State private var signing = false
    @State private var error: ITopError?
    @State private var missingFields = false
    @FocusState private var focus: Field?

    private enum Field { case server, user, password }

    var body: some View {
        @Bindable var model = model
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Sign in to iTop")
                    .font(.system(size: 20, weight: .semibold))
                    .accessibilityAddTraits(.isHeader)
                Text("Use your iTop account. Your password stays in the macOS Keychain.")
                    .foregroundStyle(Theme.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.bottom, 6)

            field("Server URL") {
                TextField("https://itop.example.com", text: $model.server)
                    .focused($focus, equals: .server)
                    .textContentType(.URL)
            }
            if isPlainHTTP {
                Text("This address isn't encrypted (http). Your password would be sent in plain text. Use https if your server supports it.")
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.warning)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, -9)
            }
            field("Username") {
                TextField("", text: $model.user)
                    .focused($focus, equals: .user)
                    .textContentType(.username)
            }
            field("Password") {
                SecureField("", text: $password)
                    .focused($focus, equals: .password)
                    .textContentType(.password)
            }

            Spacer(minLength: 0)
            message
            Button(action: submit) {
                HStack(spacing: 8) {
                    if signing { ProgressView().controlSize(.small).tint(Theme.onPrimary) }
                    Text(signing ? "Signing in…" : "Sign in to iTop")
                }
            }
            .buttonStyle(PrimaryButtonStyle())
            .keyboardShortcut(.defaultAction)
            .disabled(signing)
        }
        .textFieldStyle(.roundedBorder)
        .font(.system(size: 13))
        .padding(.horizontal, 48)
        .padding(.top, 20)
        .padding(.bottom, 32)
        .frame(width: 480, height: 520)
        .onSubmit(submit)
        .onAppear {
            focus = model.server.count <= "https://".count ? .server : (model.user.isEmpty ? .user : .password)
        }
    }

    private var isPlainHTTP: Bool {
        model.server.trimmingCharacters(in: .whitespaces).lowercased().hasPrefix("http://")
    }

    @ViewBuilder
    private var message: some View {
        if missingFields {
            Text("Enter your username and password.")
                .font(.system(size: 12)).foregroundStyle(Theme.warning)
        } else if error == .restNotAllowed {
            Text("\(Text("This account can't use the iTop REST API.").bold().foregroundStyle(Theme.warning)) Ask your iTop admin to add the \"REST Services User\" profile to \(model.user), then sign in again.")
                .font(.system(size: 12))
                .fixedSize(horizontal: false, vertical: true)
                .padding(10)
                .overlay(RoundedRectangle(cornerRadius: 8).stroke(Theme.warning))
        } else if let error {
            Text(error.localizedDescription)
                .font(.system(size: 12)).foregroundStyle(Theme.warning)
                .fixedSize(horizontal: false, vertical: true)
        } else if let notice {
            Text(notice).font(.system(size: 12)).foregroundStyle(Theme.warning)
        }
    }

    private func field<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(label).font(.system(size: 12, weight: .medium))
            content().accessibilityLabel(label)
        }
    }

    private func submit() {
        guard !signing else { return }
        error = nil
        missingFields = model.user.trimmingCharacters(in: .whitespaces).isEmpty || password.isEmpty
        if missingFields {
            focus = model.user.trimmingCharacters(in: .whitespaces).isEmpty ? .user : .password
            return
        }
        signing = true
        Task {
            defer { signing = false }
            do {
                try await model.signIn(password: password)
                password = ""
            } catch let e as ITopError {
                error = e
                if e == .invalidCredentials { focus = .password }
            } catch {
                self.error = .unexpectedResponse
            }
        }
    }
}
