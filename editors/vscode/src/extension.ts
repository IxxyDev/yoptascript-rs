import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration("yoptascript");
  const serverPath = config.get<string>("server.path")?.trim() || "yps-lsp";

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio }
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "yoptascript" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.yop")
    }
  };

  client = new LanguageClient(
    "yoptascript",
    "YoptaScript Language Server",
    serverOptions,
    clientOptions
  );

  context.subscriptions.push({ dispose: () => void client?.stop() });

  client.start().catch((err: unknown) => {
    void vscode.window.showErrorMessage(
      `YoptaScript: не удалось запустить языковой сервер '${serverPath}'. ` +
        `Укажите путь в настройке yoptascript.server.path. (${String(err)})`
    );
  });
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
