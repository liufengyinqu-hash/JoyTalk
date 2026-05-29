import React from "react";

interface SettingsGroupProps {
  title?: string;
  description?: string;
  children: React.ReactNode;
}

export const SettingsGroup: React.FC<SettingsGroupProps> = ({
  title,
  description,
  children,
}) => {
  return (
    <div className="space-y-2">
      {title && (
        <div className="px-1">
          <h2 className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider">
            {title}
          </h2>
          {description && (
            <p className="text-xs text-text-secondary mt-0.5">{description}</p>
          )}
        </div>
      )}
      <div className="surface-card overflow-visible shadow-[0_1px_2px_rgba(0,0,0,0.04)]">
        <div className="divide-y divide-border">{children}</div>
      </div>
    </div>
  );
};
