import * as vscode from "vscode";
import { existsSync } from "node:fs";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind
} from "vscode-languageclient/node";
import { resolveServerPath } from "./serverResolver.js";

let client: LanguageClient | undefined;

export function activate(context: vscode.ExtensionContext): void {
  const config = vscode.workspace.getConfiguration("yoptascript");
  const configuredPath = config.get<string>("server.path");

  const resolved = resolveServerPath({
    extensionPath: context.extensionPath,
    configuredPath,
    platform: process.platform,
    arch: process.arch,
    pathEnv: process.env.PATH,
    fileExists: existsSync
  });

  if (!resolved) {
    void vscode.window.showErrorMessage(
      "YoptaScript: не найден языковой сервер yps-lsp. Укажите путь к бинарю в настройке " +
        "yoptascript.server.path, либо соберите его из исходников " +
        "(cargo build --release -p yps-lsp) и добавьте в PATH."
    );
    return;
  }

  const serverPath = resolved.path;

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio }
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "yoptascript" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.yopta")
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
