import Foundation
import Network
import XCTest
@testable import MSIMEDoubaoTransport

/// 只绑定回环地址；由系统 WebSocket 协议栈处理升级、Ping 和关闭帧。
final class LoopbackWebSocketServer {
  enum Failure: Error { case listenerUnavailable, handshakeIncomplete }

  private let listener: NWListener
  private let queue = DispatchQueue(label: "app.msime.tests.websocket")
  private var connection: NWConnection?

  init(ready: XCTestExpectation) throws {
    let parameters = NWParameters.tcp
    parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
    let websocket = NWProtocolWebSocket.Options()
    websocket.autoReplyPing = true
    parameters.defaultProtocolStack.applicationProtocols.insert(websocket, at: 0)
    listener = try NWListener(using: parameters)
    listener.stateUpdateHandler = { [weak self] state in
      switch state {
      case .ready, .failed:
        self?.listener.stateUpdateHandler = nil
        ready.fulfill()
      default: break
      }
    }
    listener.newConnectionHandler = { [weak self] connection in
      guard let self else { connection.cancel(); return }
      self.connection = connection
      connection.start(queue: self.queue)
      self.receive(connection)
    }
    listener.start(queue: queue)
  }

  var endpoint: URL? {
    guard let port = listener.port else { return nil }
    return URL(string: "ws://127.0.0.1:\(port.rawValue)/synthetic")
  }

  func close(_ code: NWProtocolWebSocket.CloseCode) {
    queue.async { [weak self] in
      guard let connection = self?.connection else { return }
      let metadata = NWProtocolWebSocket.Metadata(opcode: .close)
      metadata.closeCode = code
      let context = NWConnection.ContentContext(identifier: "close", metadata: [metadata])
      connection.send(content: nil, contentContext: context, completion: .contentProcessed { _ in })
    }
  }

  func stop() {
    queue.sync {
      connection?.cancel()
      connection = nil
      listener.cancel()
    }
  }

  private func receive(_ connection: NWConnection) {
    connection.receiveMessage { [weak self, weak connection] data, context, _, error in
      guard let self, let connection else { return }
      guard error == nil else { connection.cancel(); return }
      let metadata = context?.protocolMetadata(definition: NWProtocolWebSocket.definition)
        as? NWProtocolWebSocket.Metadata
      if metadata?.opcode == .close {
        connection.cancel()
        return
      }
      if metadata?.opcode == .binary, let data {
        let response = NWProtocolWebSocket.Metadata(opcode: .binary)
        let context = NWConnection.ContentContext(identifier: "echo", metadata: [response])
        connection.send(content: data, contentContext: context, completion: .contentProcessed { _ in })
      }
      self.receive(connection)
    }
  }
}

extension XCTestCase {
  func makeLoopbackServer() async throws -> LoopbackWebSocketServer {
    let ready = expectation(description: "回环 WebSocket 服务就绪")
    let server = try LoopbackWebSocketServer(ready: ready)
    await fulfillment(of: [ready], timeout: 5)
    guard server.endpoint != nil else {
      server.stop()
      throw LoopbackWebSocketServer.Failure.listenerUnavailable
    }
    return server
  }

  func connect(_ transport: DoubaoWebSocketTransport, to server: LoopbackWebSocketServer) async throws {
    let completed = expectation(description: "真实 WebSocket 握手和 Ping 完成")
    let start = Task {
      defer { completed.fulfill() }
      try await transport.start(endpoint: server.endpoint!)
    }
    let result = await XCTWaiter.fulfillment(of: [completed], timeout: 5)
    guard result == .completed else {
      XCTFail("真实 WebSocket 握手未在截止时间内完成")
      start.cancel()
      transport.cancel()
      server.stop()
      throw LoopbackWebSocketServer.Failure.handshakeIncomplete
    }
    try await start.value
  }

  func waitForTransport(_ description: String, timeout: TimeInterval = 5, condition: @escaping () -> Bool) async {
    let changed = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in condition() }, object: nil)
    changed.expectationDescription = description
    await fulfillment(of: [changed], timeout: timeout)
  }

  func closeRemotely(_ transport: DoubaoWebSocketTransport, server: LoopbackWebSocketServer,
                     code: NWProtocolWebSocket.CloseCode) async {
    let completed = expectation(description: "接收方处理远端关闭帧")
    let receive = Task {
      defer { completed.fulfill() }
      _ = try? await transport.receive()
    }
    server.close(code)
    let result = await XCTWaiter.fulfillment(of: [completed], timeout: 5)
    guard result == .completed else {
      XCTFail("接收方未在截止时间内处理远端关闭帧")
      receive.cancel()
      transport.cancel()
      server.stop()
      return
    }
    await receive.value
    await waitForTransport("收到真实远端关闭回调") { !transport.isConnected }
    if transport.isConnected { transport.cancel() }
  }
}
