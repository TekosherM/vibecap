import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  isNativeCaptureTool,
  nativeCliArgs,
  wantsNativeSource,
} from "./native-capture.ts";

describe("native capture connector", () => {
  it("treats display / window / output_dir as the real screen", () => {
    assert.equal(wantsNativeSource({}), false);
    assert.equal(wantsNativeSource({ source: "demo" }), false);
    assert.equal(wantsNativeSource({ display: ":1" }), true);
    assert.equal(wantsNativeSource({ window: "Chrome" }), true);
    assert.equal(wantsNativeSource({ output_dir: "/workspace/run4/frames" }), true);
    assert.equal(wantsNativeSource({ source: "x11grab" }), true);
  });

  it("only native-routes capture tools", () => {
    assert.equal(isNativeCaptureTool("vibecap_capture"), true);
    assert.equal(isNativeCaptureTool("vibecap_record_start"), true);
    assert.equal(isNativeCaptureTool("vibecap_job"), false);
    assert.equal(isNativeCaptureTool("vibecap_ingest_frontend"), false);
  });

  it("maps tools onto the same CLI flags as the binary", () => {
    assert.deepEqual(
      nativeCliArgs("vibecap_capture", {
        display: ":1",
        output_dir: "/tmp/frames",
        window: "QuestOS Search",
      }),
      [
        "--screenshot",
        "--output-dir",
        "/tmp/frames",
        "--display",
        ":1",
        "--window",
        "QuestOS Search",
      ],
    );
    assert.deepEqual(nativeCliArgs("vibecap_record_start", { gif: true, display: ":0" }), [
      "--record-start",
      "--display",
      ":0",
      "--gif",
    ]);
    assert.deepEqual(nativeCliArgs("vibecap_record_stop", {}), ["--record-stop"]);
  });
});
