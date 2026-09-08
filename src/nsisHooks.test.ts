/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";

import hooks from "../src-tauri/nsis-hooks.nsh?raw";
import tauriConfig from "../src-tauri/tauri.conf.json?raw";

describe("NSIS uninstall cleanup integration", () => {
  it("runs application cleanup before uninstall and honors the delete-data checkbox", () => {
    expect(tauriConfig).toContain('"installerHooks": "nsis-hooks.nsh"');
    expect(hooks).toContain("NSIS_HOOK_PREUNINSTALL");
    expect(hooks).toContain("$DeleteAppDataCheckboxState = 1");
    expect(hooks).toContain("--uninstall-cleanup --purge");
    expect(hooks).toContain("--uninstall-cleanup'");
  });
});
