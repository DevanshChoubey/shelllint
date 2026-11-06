// SPDX-License-Identifier: GPL-3.0-or-later
import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions, TransportKind } from 'vscode-languageclient/node';

const SECTION = 'shellcheck-rust';
let client: LanguageClient | undefined;

function serverCommand(): string {
  const configured = vscode.workspace.getConfiguration(SECTION).get<string>('server.path', '').trim();
  return configured.length > 0 ? configured : 'shellcheck-rust-lsp';
}

/** The settings the server reads: the flat section, so the same shape is
 *  used for initializationOptions and didChangeConfiguration. */
function settings(): Record<string, unknown> {
  const cfg = vscode.workspace.getConfiguration(SECTION);
  return {
    dialect: cfg.get<string>('dialect', 'auto'),
    enable: cfg.get<string[]>('enable', []),
    exclude: cfg.get<string[]>('exclude', []),
    severity: cfg.get<string>('severity', 'style'),
  };
}

async function startClient(): Promise<void> {
  const command = serverCommand();
  const serverOptions: ServerOptions = {
    run: { command, args: [], transport: TransportKind.stdio },
    debug: { command, args: [], transport: TransportKind.stdio },
  };
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: 'file', language: 'shellscript' },
      { scheme: 'untitled', language: 'shellscript' },
    ],
    initializationOptions: settings(),
    synchronize: { configurationSection: SECTION },
  };
  client = new LanguageClient(SECTION, 'ShellCheck (Rust)', serverOptions, clientOptions);
  try {
    await client.start();
  } catch (err) {
    client = undefined;
    const msg = err instanceof Error ? err.message : String(err);
    vscode.window.showErrorMessage(
      `ShellCheck (Rust): could not start "${command}". Set shellcheck-rust.server.path to the binary. (${msg})`,
    );
  }
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand('shellcheck-rust.restart', async () => {
      if (client) {
        await client.stop();
        client = undefined;
      }
      await startClient();
    }),
  );
  await startClient();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}
