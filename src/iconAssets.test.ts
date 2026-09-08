/// <reference types="vite/client" />

import { describe, expect, it } from "vitest";

import icon32 from "../src-tauri/icons/32x32.png?url";
import icon128 from "../src-tauri/icons/128x128.png?url";
import icon256 from "../src-tauri/icons/128x128@2x.png?url";
import windowsIcon from "../src-tauri/icons/icon.ico?url";

describe("Tauri application icons", () => {
  it("resolves every desktop icon required by Tauri", () => {
    expect([icon32, icon128, icon256, windowsIcon]).toEqual([
      expect.stringContaining("32x32.png"),
      expect.stringContaining("128x128.png"),
      expect.stringContaining("128x128@2x.png"),
      expect.stringContaining("icon.ico"),
    ]);
  });
});
