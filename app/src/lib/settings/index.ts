import type { NodeInput } from '@nodarium/types';

type Button = { type: 'button'; label?: string };

export type SettingsStore = {
  [key: string]: SettingsStore | string | number | boolean;
};

type InputType = NodeInput | Button;

type SettingsNode = InputType | SettingsGroup;

export interface SettingsGroup {
  title?: string;
  [key: string]: SettingsNode | string | number | undefined;
}

export type SettingsType = Record<string, SettingsNode>;

export type SettingsValue = Record<
  string,
  Record<string, unknown> | string | number | boolean | number[]
>;

export function isNodeInput(v: SettingsNode | undefined): v is InputType {
  return !!v && 'type' in v;
}
