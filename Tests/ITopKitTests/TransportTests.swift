import Foundation
import Testing
@testable import ITopKit

// MARK: - Server address

@Suite("Server address")
struct ServerAddressTests {
    @Test("A bare host name becomes https")
    func bareHostIsHTTPS() throws {
        let address = try ServerAddress("itop.example.com")
        #expect(address.baseURL.absoluteString == "https://itop.example.com")
        #expect(address.isSecure)
    }

    @Test("An http address is accepted but flagged as not secure")
    func httpIsFlagged() throws {
        let address = try ServerAddress("http://itop.example.com")
        #expect(address.isSecure == false)
    }

    @Test("Pasting an iTop page URL keeps only the server and its folder")
    func pastedPageURLIsTrimmed() throws {
        let address = try ServerAddress("https://itop.example.com/itop/pages/UI.php?operation=details&class=UserRequest&id=4")
        #expect(address.baseURL.absoluteString == "https://itop.example.com/itop")
    }

    @Test("Pasting the REST endpoint itself works too")
    func pastedRESTURLIsTrimmed() throws {
        let address = try ServerAddress("https://itop.example.com/webservices/rest.php?version=1.3")
        #expect(address.baseURL.absoluteString == "https://itop.example.com")
    }

    @Test("Spaces and a trailing slash are ignored")
    func whitespaceAndSlash() throws {
        let address = try ServerAddress("  https://itop.example.com/  ")
        #expect(address.baseURL.absoluteString == "https://itop.example.com")
    }

    @Test("The REST endpoint uses version 1.4")
    func endpoint() throws {
        let address = try ServerAddress("https://itop.example.com/itop")
        #expect(address.restEndpoint.absoluteString == "https://itop.example.com/itop/webservices/rest.php?version=1.4")
    }

    @Test("Other schemes and empty input are rejected", arguments: ["ftp://itop.example.com", "", "   ", "https://"])
    func rejected(_ input: String) {
        #expect(throws: ITopError.invalidServerAddress) { try ServerAddress(input) }
    }
}

// MARK: - Request encoding

@Suite("Request encoding")
struct RequestEncodingTests {
    @Test("send posts a form with auth_user, auth_pwd and json_data")
    func formBody() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":null}"#))
        let client = try ITopClient(server: ServerAddress("https://itop.example.com"),
                                    credentials: Credentials(user: "agent", password: "s3cret"),
                                    session: stub.session)

        _ = try await client.send(.get(className: "UserRequest", key: "SELECT UserRequest WHERE agent_id = 7",
                                       outputFields: ["ref", "title"]))

        let request = try #require(stub.requests.first)
        #expect(request.request.httpMethod == "POST")
        #expect(request.request.url?.absoluteString == "https://itop.example.com/webservices/rest.php?version=1.4")
        #expect(request.request.value(forHTTPHeaderField: "Content-Type") == "application/x-www-form-urlencoded")
        #expect(request.form["auth_user"] == "agent")
        #expect(request.form["auth_pwd"] == "s3cret")
        #expect(request.jsonData["operation"] as? String == "core/get")
        #expect(request.jsonData["class"] as? String == "UserRequest")
        #expect(request.jsonData["key"] as? String == "SELECT UserRequest WHERE agent_id = 7")
        #expect(request.jsonData["output_fields"] as? String == "ref,title")
    }

    @Test("Special characters in the password and the query survive form encoding")
    func specialCharacters() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":""}"#))
        let password = "p&ss=w+rd %20 é?#"
        let oql = "SELECT UserRequest WHERE title LIKE '%VPN & Wi-Fi%'"
        let client = try ITopClient(server: ServerAddress("itop.example.com"),
                                    credentials: Credentials(user: "agent@corp", password: password),
                                    session: stub.session)

        _ = try await client.send(.get(className: "UserRequest", key: oql, outputFields: ["*"]))

        let request = try #require(stub.requests.first)
        #expect(request.form["auth_user"] == "agent@corp")
        #expect(request.form["auth_pwd"] == password)
        #expect(request.jsonData["key"] as? String == oql)
    }

    @Test("check_credentials sends the user and password in json_data")
    func checkCredentialsShape() async throws {
        let stub = StubServer(.response(status: 200, contentType: "application/json", body: try Fixture.data("check_credentials")))
        let client = try ITopClient(server: ServerAddress("itop.example.com"),
                                    credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        let authorized = try await client.checkCredentials()

        #expect(authorized)
        let json = try #require(stub.requests.first).jsonData
        #expect(json["operation"] as? String == "core/check_credentials")
        #expect(json["user"] as? String == "agent")
        #expect(json["password"] as? String == "pw")
    }
}

// MARK: - Responses and errors

@Suite("Responses and errors")
struct ResponseTests {
    func client(_ reply: StubReply) throws -> (ITopClient, StubServer) {
        let stub = StubServer(reply)
        let client = try ITopClient(server: ServerAddress("itop.example.com"),
                                    credentials: Credentials(user: "agent", password: "pw"), session: stub.session)
        return (client, stub)
    }

    @Test("A real ticket fixture decodes into objects with their fields")
    func decodesFixture() async throws {
        let (client, _) = try client(.response(status: 200, contentType: "application/json", body: try Fixture.data("userrequest")))

        let response = try await client.send(.get(className: "UserRequest", key: "SELECT UserRequest", outputFields: ["*"]))

        let object = try #require(response.objects.first)
        #expect(object.className == "UserRequest")
        #expect(object.fields["status"] == .string("assigned"))
        #expect(object.fields["public_log"]?["entries"] != nil)
    }

    @Test("iTop code 1 means the username or password is wrong")
    func unauthorized() async throws {
        let (client, _) = try client(.json(#"{"code":1,"message":"Error: Invalid login"}"#))
        await #expect(throws: ITopError.invalidCredentials) {
            _ = try await client.send(.listOperations)
        }
    }

    @Test("Other iTop codes keep their number and message",
          arguments: [(3, "Error: Missing json_data"), (11, "Error: Unknown operation"), (100, "Error: Internal error")])
    func serverErrors(code: Int, message: String) async throws {
        let (client, _) = try client(.json(#"{"code":\#(code),"message":"\#(message)"}"#))
        await #expect(throws: ITopError.server(code: code, message: message)) {
            _ = try await client.send(.listOperations)
        }
    }

    @Test("An HTML page instead of JSON (a login page or proxy) is reported as unexpected")
    func htmlIsUnexpected() async throws {
        let (client, _) = try client(.response(status: 200, contentType: "text/html", body: Data("<html><body>Login</body></html>".utf8)))
        await #expect(throws: ITopError.unexpectedResponse) {
            _ = try await client.send(.listOperations)
        }
    }

    @Test("HTTP errors are reported with their status")
    func httpStatus() async throws {
        let (client, _) = try client(.json("{}", status: 503))
        await #expect(throws: ITopError.http(status: 503)) {
            _ = try await client.send(.listOperations)
        }
    }

    @Test("Network failures become transport errors")
    func transport() async throws {
        let (client, _) = try client(.failure(URLError(.notConnectedToInternet)))
        await #expect(throws: ITopError.transport(URLError.Code.notConnectedToInternet.rawValue)) {
            _ = try await client.send(.listOperations)
        }
    }

    @Test("A certificate the Mac doesn't trust is reported as such, not as a generic network error")
    func untrustedCertificate() async throws {
        let (client, _) = try client(.failure(URLError(.serverCertificateUntrusted)))
        await #expect(throws: ITopError.untrustedCertificate) {
            _ = try await client.send(.listOperations)
        }
    }
}

// MARK: - Read-only guard

@Suite("Read-only guard")
struct ReadOnlyGuardTests {
    @Test("Write operations are refused before anything is sent while writes are off",
          arguments: [
              RESTOperation.update(className: "UserRequest", key: "1", fields: ["title": .string("x")], comment: "iTop Wrap"),
              RESTOperation.create(className: "UserRequest", fields: ["title": .string("x")], comment: "iTop Wrap"),
              RESTOperation.applyStimulus(className: "UserRequest", key: "1", stimulus: "ev_resolve", fields: [:], comment: "iTop Wrap"),
          ])
    func writesRefused(_ operation: RESTOperation) async throws {
        let stub = StubServer(.json(#"{"code":0,"message":""}"#))
        let client = try ITopClient(server: ServerAddress("itop.example.com"),
                                    credentials: Credentials(user: "agent", password: "pw"), session: stub.session)

        await #expect(throws: ITopError.writesDisabled) { _ = try await client.send(operation) }
        #expect(stub.requests.isEmpty)
    }

    @Test("Read operations are allowed while writes are off",
          arguments: [RESTOperation.listOperations, .get(className: "Person", key: "1", outputFields: ["name"])])
    func readsAllowed(_ operation: RESTOperation) {
        #expect(operation.isWrite == false)
    }

    @Test("Writes go through once they're switched on, with the comment attached")
    func writesWhenEnabled() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"","objects":{}}"#))
        let client = try ITopClient(server: ServerAddress("itop.example.com"),
                                    credentials: Credentials(user: "agent", password: "pw"),
                                    session: stub.session, allowsWrites: true)

        _ = try await client.send(.update(className: "UserRequest", key: "1", fields: ["title": .string("x")], comment: "iTop Wrap"))

        let json = try #require(stub.requests.first).jsonData
        #expect(json["operation"] as? String == "core/update")
        #expect(json["comment"] as? String == "iTop Wrap")
        #expect((json["fields"] as? [String: Any])?["title"] as? String == "x")
    }
}

// MARK: - Debug trace

@Suite("Debug trace")
struct TraceTests {
    @Test("A trace line shows the operation, class, query and what iTop found, never the credentials")
    func line() throws {
        let body = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"code":0,"message":"Found: 7","objects":{"UserRequest::1":{"class":"UserRequest","key":"1","fields":{"title":"Secret title"}}}}"#.utf8))
        let line = ITopClient.traceLine(.get(className: "UserRequest", key: "SELECT UserRequest WHERE org_id = 5", outputFields: ["ref"]),
                                        response: ITopResponse(body: body))
        #expect(line == "[iTop] core/get UserRequest | SELECT UserRequest WHERE org_id = 5 | code 0 | 1 objects | Found: 7")
        #expect(!line.contains("Secret"))
    }

    @Test("check_credentials never shows the user or password")
    func credentials() throws {
        let body = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"code":0,"message":"","authorized":true}"#.utf8))
        let line = ITopClient.traceLine(.checkCredentials(user: "agent", password: "s3cret"), response: ITopResponse(body: body))
        #expect(!line.contains("s3cret") && !line.contains("agent"))
    }
}

@Suite("Found count")
struct FoundCountTests {
    @Test("iTop's \"Found: N\" message gives the total even when fewer objects came back", arguments: [
        ("Found: 256", 256), ("Found: 0", 0), ("", nil), ("Scope: 1; Found: Server=4, VirtualMachine=3", nil),
    ] as [(String, Int?)])
    func found(message: String, expected: Int?) throws {
        let body = try JSONDecoder().decode(JSONValue.self, from: Data(#"{"code":0,"message":"\#(message)"}"#.utf8))
        #expect(ITopResponse(body: body).found == expected)
    }

    @Test("People search reports how many matched in total")
    func peopleTotal() async throws {
        let stub = StubServer(.json(#"{"code":0,"message":"Found: 256","objects":{"Person::1":{"class":"Person","key":"1","fields":{"name":"Rajasyah","first_name":"Alfi"}}}}"#))
        let client = ITopClient(server: try ServerAddress("itop.example.com"), credentials: Credentials(user: "a", password: "p"), session: stub.session)
        let page = try await OrgService(client: client).peoplePage(matching: "alfi")
        #expect(page.people.count == 1)
        #expect(page.total == 256)
    }
}
