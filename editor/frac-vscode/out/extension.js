"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode_1 = require("vscode");
const node_1 = require("vscode-languageclient/node");
let client;
function resolveServerPath() {
    const fromConfig = vscode_1.workspace.getConfiguration("frac").get("serverPath");
    if (fromConfig && fromConfig.length > 0)
        return fromConfig;
    if (process.env.FRAC_LSP_PATH)
        return process.env.FRAC_LSP_PATH;
    return "frac_lang_lsp";
}
function activate(context) {
    const command = resolveServerPath();
    const serverOptions = {
        run: { command, transport: node_1.TransportKind.stdio },
        debug: { command, transport: node_1.TransportKind.stdio },
    };
    const clientOptions = {
        documentSelector: [{ scheme: "file", language: "frac" }],
        synchronize: {
            fileEvents: vscode_1.workspace.createFileSystemWatcher("**/*.frac"),
        },
    };
    client = new node_1.LanguageClient("fracLanguageServer", "frac language server", serverOptions, clientOptions);
    client.start().catch((err) => {
        vscode_1.window.showErrorMessage(`frac_lang_lsp failed to start: ${err}`);
    });
}
function deactivate() {
    return client?.stop();
}
//# sourceMappingURL=extension.js.map