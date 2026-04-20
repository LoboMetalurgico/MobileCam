import { Role } from "./interfaces/Role";
import { ServerToClient as ServerToStreamer } from "./protos/streamer";
import { ServerToClient as ServerToViewer } from "./protos/viewer";
import { Ready, ServerToClient as ServerToController } from "./protos/controller";

const roleToServerMap = {
  [Role.Streamer]: ServerToStreamer,
  [Role.Viewer]: ServerToViewer,
  [Role.Controller]: ServerToController,
};

export class WebsocketManager<T extends keyof typeof roleToServerMap> extends EventTarget {
  private hostProtocol?: string;
  private hostname?: string;
  private socket?: WebSocket;
  private commandParser: (command: ServerToStreamer | ServerToViewer | ServerToController) => void;
  private extraQueries?: [string, string | null][];
  private retryCount = 0;
  private isRetrying = false;
  private isExiting = false;
  private messagePool: (Uint8Array | string)[] = [];
  private role: T;
  private isControllerReady = false;

  constructor(role: T, commandParser: (command: ServerToStreamer | ServerToViewer | ServerToController) => void) {
    super();
    this.role = role;
    this.commandParser = commandParser;
  }

  async connect(host: { protocol: string, hostname: string, extraQueries?: [string, string | null][] }) {
    this.hostProtocol = host.protocol;
    this.hostname = host.hostname;
    this.extraQueries = host.extraQueries;

    const wsProtocol = this.hostProtocol === "https:" ? "wss:" : "ws:";
    const queries = this.extraQueries?.filter((item) => item[1] != null).map(([k, v]) => `${k}=${v}`).join("&");
    this.socket = new WebSocket(
      `${wsProtocol}//${this.hostname}/ws?role=${this.role}` + (queries ? "&" + queries : ""),
    );

    this.socket.addEventListener('message', (event) => { this.onMessage(event.data) });
    this.socket.addEventListener('error', this.onError.bind(this));
    this.socket.addEventListener('close', this.onClose.bind(this));
    this.socket.addEventListener('open', () => {
      this.onOpen();
      return Promise.resolve();
    });
  }

  public finish() {
    this.isExiting = true;
    if (this.socket) {
      this.socket.close();
      this.socket = undefined;
    }
  }

  private onOpen() {
    console.log('Connection established');
    this.retryCount = 0;
    // Send any queued messages
    while (this.messagePool.length > 0) {
      const message = this.messagePool.shift();
      if (message) {
        this.send(message);
      }
    }
  }

  private onMessage(message: Blob | string) {
    if (message instanceof Blob) {
      const reader = new FileReader();
      reader.onload = () => {
        const arrayBuffer = reader.result as ArrayBuffer;
        const uint8Array = new Uint8Array(arrayBuffer);
        this.onMessageParsed(uint8Array);
      };
      reader.readAsArrayBuffer(message);
    } else {
      const uint8Array = new TextEncoder().encode(message);
      this.onMessageParsed(uint8Array);
    }
  }

  private onMessageParsed(message: Uint8Array) {
    if (!this.isControllerReady && this.role === Role.Controller) {
      this.isControllerReady = true;
      const data = Ready.decode(message);
      this.dispatchEvent(new CustomEvent("ready", { detail: data }));
      return;
    }
    const parsedMsg = roleToServerMap[this.role].decode(new Uint8Array(message));
    this.commandParser(parsedMsg);
  }

  private onError() {
    console.log('Connection error');
  }

  private retryConnection() {
    if (this.retryCount < 10 && !this.isRetrying && !this.isExiting) {
      this.isRetrying = true;
      console.log(`Attempting to reconnect... (attempt ${this.retryCount + 1})`);
      setTimeout(async () => {
        this.isRetrying = false;
        await this.connect({
          protocol: this.hostProtocol!,
          hostname: this.hostname!,
          extraQueries: this.extraQueries,
        });
      }, Math.min(1000 * this.retryCount, 30000)); // cap at 30 seconds
      this.retryCount++;
    }
  }

  private onClose() {
    console.log('Connection closed');
    this.retryConnection();
  }

  public send(message: Uint8Array | string) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      if (typeof message === "string") {
        this.socket.send(new TextEncoder().encode(message).buffer);
      } else {
        this.socket.send(Uint8Array.from(message).buffer);
      }
    } else {
      this.messagePool.push(message);
    }
  }
}