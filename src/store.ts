import { Store } from "@tauri-apps/plugin-store";

export interface AppSettings {
  selected_microphone: string | null;
  word_count: number;
  hotkey: string | null;
}

let _store: Promise<Store> | undefined;
const store = () => {
  if (!_store) {
    _store = Store.load("settings");
  }
  return _store;
};

export const appStore = {
  get: () => store().then((s) => s.get<AppSettings>("app_settings")),
  set: async (value: AppSettings) => {
    const s = await store();
    await s.set("app_settings", value);
    await s.save();
  },
  listen: (fn: (data: AppSettings | null) => void) =>
    store().then((s) => s.onKeyChange<AppSettings>("app_settings", (value) => fn(value || null))),
  initialize: async () => {
    try {
      const s = await store();
      const currentSettings = await s.get<AppSettings>("app_settings");
      if (currentSettings === undefined) {
        const defaultSettings: AppSettings = {
          selected_microphone: null,
          word_count: 0,
          hotkey: null,
        };
        await s.set("app_settings", defaultSettings);
        await s.save();
        return defaultSettings;
      }
      return currentSettings || null;
    } catch (error) {
      console.error("[appStore] Initialization error:", error);
      return null;
    }
  },
};

let storeInitialized = false;

export const initializeStore = async () => {
  if (storeInitialized) {
    return;
  }
  
  try {
    await appStore.initialize();
    storeInitialized = true;
  } catch (error) {
    console.error("Failed to initialize store:", error);
    throw error;
  }
};