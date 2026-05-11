import * as path from "path";
import { ExtensionContext, workspace, window } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

function resolveServerPath(): string {
  const fromConfig = workspace.getConfiguration("frac").get<string>("serverPath");
  if (fromConfig && fromConfig.length > 0) return fromConfig;
  if (process.env.FRAC_LSP_PATH) return process.env.FRAC_LSP_PATH;
  return "frac_lang_lsp";
}

export function activate(context: ExtensionContext) {
  const command = resolveServerPath();

  const serverOptions: ServerOptions = {
    run: { command, transport: TransportKind.stdio },
    debug: { command, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "frac" }],
    synchronize: {
      fileEvents: workspace.createFileSystemWatcher("**/*.frac"),
    },
  };

  client = new LanguageClient(
    "fracLanguageServer",
    "frac language server",
    serverOptions,
    clientOptions
  );

  client.start().catch((err) => {
    window.showErrorMessage(`frac_lang_lsp failed to start: ${err}`);
  });
}

export function deactivate(): Thenable<void> | undefined {
  return client?.stop();
}
