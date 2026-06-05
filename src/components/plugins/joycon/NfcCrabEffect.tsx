import React, { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { ClaudeCrabIcon } from "./ClaudeCrabIcon";
import "./NfcCrabEffect.css";

interface NfcLiveSample {
  session_active: boolean;
  tag_present: boolean;
  uid: string;
  uid_len: number;
  tag_type: number;
  nfc_state: number;
}

function isReadyToRescan(state: number): boolean {
  return state === 0x00 || state === 0x01;
}

/** Full-screen hop: Claude Code crab bounces right → left on each new NFC UID. */
export const NfcCrabEffect: React.FC = () => {
  const [burst, setBurst] = useState(0);
  const lastUidRef = useRef("");

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<NfcLiveSample>("joycon://nfc_sample", (event) => {
      const sample = event.payload;
      if (sample.tag_present && sample.uid && sample.nfc_state === 0x09) {
        if (sample.uid !== lastUidRef.current) {
          lastUidRef.current = sample.uid;
          setBurst((n) => n + 1);
        }
        return;
      }
      if (!sample.tag_present && isReadyToRescan(sample.nfc_state)) {
        lastUidRef.current = "";
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  if (burst === 0) return null;

  return (
    <div key={burst} className="nfc-crab-effect" aria-hidden>
      <ClaudeCrabIcon className="nfc-crab-effect__icon" />
    </div>
  );
};
