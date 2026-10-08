export type FlightLogFormat = "csv" | "json";

export interface FlightLogExportPort {
  export_flight_log_csv(): string;
  export_current_flight_record_json(): string;
}

export interface FlightLogDownload {
  readonly text: string;
  readonly format: FlightLogFormat;
  readonly filename: string;
}

export interface FlightLogDownloadPort {
  download(log: FlightLogDownload): void;
  dispose(): void;
}

export function readFlightLog(port: FlightLogExportPort, format: FlightLogFormat): string {
  return format === "csv" ? port.export_flight_log_csv() : port.export_current_flight_record_json();
}
