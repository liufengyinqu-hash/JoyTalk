import React from "react";
import { PluginManifest } from "./manifest";

interface Props {
  plugin: PluginManifest;
  installed?: boolean;
  onOpen: () => void;
}

export const PluginCard: React.FC<Props> = ({ plugin, installed, onOpen }) => {
  const canOpen = !plugin.comingSoon;
  return (
    <button
      type="button"
      onClick={canOpen ? onOpen : undefined}
      disabled={!canOpen}
      className={`surface-card text-left p-4 flex flex-col gap-2 transition-all ${
        canOpen
          ? "hover:border-accent/40 hover:shadow-md cursor-pointer"
          : "opacity-60 cursor-not-allowed"
      }`}
    >
      <div className="flex items-start justify-between">
        <div className="w-10 h-10 rounded-mac bg-accent/10 flex items-center justify-center text-2xl">
          {plugin.icon}
        </div>
        {plugin.comingSoon ? (
          <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-text/10 text-text-secondary">
            Coming soon
          </span>
        ) : installed ? (
          <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-success/15 text-success">
            Installed
          </span>
        ) : (
          <span className="text-[10px] font-medium px-2 py-0.5 rounded-full bg-accent/15 text-accent">
            Built-in
          </span>
        )}
      </div>
      <div>
        <div className="text-[14px] font-semibold">{plugin.name}</div>
        <div className="text-[12px] text-text-secondary mt-1 leading-snug">
          {plugin.description}
        </div>
      </div>
      <div className="flex flex-wrap gap-1 mt-1">
        {plugin.capabilities.map((c) => (
          <span
            key={c}
            className="text-[10px] px-1.5 py-0.5 rounded bg-text/5 text-text-secondary"
          >
            {c}
          </span>
        ))}
      </div>
    </button>
  );
};
