import type { Role } from "./interfaces/Role";

export class WebsocketManager {
  private hostProtocol?: string;
  private hostname?: string;
  private role: Role;
  private socket?: WebSocket;
  private commandParser: (command: string, params: string) => void;
  private extraQueries?: [string, string][];
  private retryCount = 0;
  private isRetrying = false;
  private isExiting = false;
  private messagePool: string[] = [];

  constructor(role: Role, commandParser: (command: string, params: string) => void) {
    this.role = role;
    this.commandParser = commandParser;
  }

  async connect(host: { protocol: string, hostname: string, extraQueries?: [string, string][] }) {
    this.hostProtocol = host.protocol;
    this.hostname = host.hostname;
    this.extraQueries = host.extraQueries;

    const wsProtocol = this.hostProtocol === "https:" ? "wss:" : "ws:";
    this.socket = new WebSocket(
      `${wsProtocol}//${this.hostname}/ws?role=${this.role}` + (this.extraQueries ? "&" + this.extraQueries.map(([k, v]) => `${k}=${v}`).join("&") : ""),
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

  private onMessage(message: ArrayBuffer) {
    const commandStr: string = message.toString();
    const parts = commandStr.split(":");
    const command = parts.shift();
    const params = parts.join(":");

    if (command) {
      this.commandParser(command, params);
    } else {
      console.warn('Received invalid command:', commandStr);
    }
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
      }, Math.min(1000 * (2 ** this.retryCount), 30000)); // cap at 30 seconds
      this.retryCount++;
    }
  }

  private onClose() {
    console.log('Connection closed');
    this.retryConnection();
  }

  public send(message: string) {
    if (this.socket && this.socket.readyState === WebSocket.OPEN) {
      this.socket.send(message);
    } else {
      this.messagePool.push(message);
    }
  }
}