const Module = require('module');

class MockCancellationTokenSource {
  constructor() {
    this._isCancelled = false;
    this._listeners = [];
    const self = this;
    this.token = {
      get isCancellationRequested() {
        return self._isCancelled;
      },
      onCancellationRequested(listener) {
        self._listeners.push(listener);
        return { dispose: () => {} };
      },
      _source: self,
    };
  }

  cancel() {
    this._isCancelled = true;
    for (const listener of this._listeners) {
      try {
        listener();
      } catch {
        // ignore errors
      }
    }
  }

  dispose() {
    this._listeners = [];
  }
}

class MockOutputChannel {
  constructor(name) {
    this.name = name;
    this.lines = [];
  }
  append(val) { this.lines.push(val); }
  appendLine(val) { this.lines.push(val); }
  clear() { this.lines = []; }
  show(preserveFocus) {}
  hide() {}
  dispose() {}
}

class MockStatusBarItem {
  constructor() {
    this.text = '';
    this.tooltip = '';
    this.command = '';
    this.backgroundColor = undefined;
    this.color = undefined;
    this.visible = false;
  }
  show() { this.visible = true; }
  hide() { this.visible = false; }
  dispose() {}
}

class MockThemeColor {
  constructor(id) {
    this.id = id;
  }
}

class MockThemeIcon {
  constructor(id, color) {
    this.id = id;
    this.color = color;
  }
}

class MockDisposable {
  constructor(callOnDispose) {
    this.callOnDispose = callOnDispose;
  }
  dispose() {
    if (this.callOnDispose) {
      this.callOnDispose();
    }
  }
  static from(...disposables) {
    return new MockDisposable(() => disposables.forEach((d) => d.dispose()));
  }
}

class MockMarkdownString {
  constructor(value) {
    this.value = value || '';
  }
}

class MockEventEmitter {
  constructor() {
    this._listeners = [];
    this.event = (listener) => {
      this._listeners.push(listener);
      return { dispose: () => {} };
    };
  }
  fire(data) {
    for (const l of this._listeners) {
      l(data);
    }
  }
  dispose() {
    this._listeners = [];
  }
}

const mockVscode = {
  window: {
    createOutputChannel: (name) => new MockOutputChannel(name),
    createStatusBarItem: (alignment, priority) => new MockStatusBarItem(),
    showInformationMessage: async (message, ...items) => items[0],
    showWarningMessage: async (message, ...items) => items[0],
    showErrorMessage: async (message, ...items) => items[0],
    withProgress: async (options, task) => {
      const cts = new MockCancellationTokenSource();
      return task({ report: () => {} }, cts.token);
    },
  },
  StatusBarAlignment: {
    Left: 1,
    Right: 2,
  },
  ThemeColor: MockThemeColor,
  ThemeIcon: MockThemeIcon,
  commands: {
    registerCommand: (command, callback) => ({
      dispose: () => {},
    }),
    executeCommand: async (command, ...args) => {},
  },
  workspace: {
    getConfiguration: (section) => ({
      get: (key, defaultValue) => defaultValue,
      update: async () => {},
    }),
    onDidChangeConfiguration: (listener) => ({ dispose: () => {} }),
    workspaceFolders: [],
  },
  ProgressLocation: {
    SourceControl: 1,
    Window: 10,
    Notification: 15,
  },
  MarkdownString: MockMarkdownString,
  CancellationTokenSource: MockCancellationTokenSource,
  Disposable: MockDisposable,
  env: {
    openExternal: async (uri) => true,
  },
  Uri: {
    file: (fsPath) => ({ fsPath, scheme: 'file', toString: () => fsPath }),
    parse: (str) => ({ fsPath: str, scheme: 'https', toString: () => str }),
  },
  EventEmitter: MockEventEmitter,
};

// Intercept require('vscode')
const origResolveFilename = Module._resolveFilename;

Module._resolveFilename = function (request, ...args) {
  if (request === 'vscode') {
    return 'vscode';
  }
  return origResolveFilename.call(this, request, ...args);
};

require.cache['vscode'] = {
  id: 'vscode',
  filename: 'vscode',
  loaded: true,
  exports: mockVscode,
};

module.exports = { mockVscode };
