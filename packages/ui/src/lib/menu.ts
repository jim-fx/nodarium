export type MenuAction = {
  label: string;
  /** shown muted on the right, e.g. "Ctrl+O" */
  shortcut?: string;
  /** shown muted on the right instead of the shortcut, e.g. a date */
  detail?: string;
  /** iconify class, e.g. "i-[tabler--download]" */
  icon?: string;
  /** thumbnail url, an empty string renders an empty thumbnail frame */
  image?: string;
  /** renders the item as a checkbox item */
  checked?: boolean;
  disabled?: boolean;
  onselect?: () => void;
  /** renders a submenu */
  items?: MenuEntry[];
};

export type MenuEntry = MenuAction | 'separator';

export type Menu = {
  label: string;
  items: MenuEntry[];
};
