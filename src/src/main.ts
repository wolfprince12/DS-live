import "./style.css";
import { store } from "./store";
import { initUI } from "./ui";

store.setTheme(store.theme);
initUI();
