{{/* ClickHouse host: explicit value, or the operator-managed headless service
     (<release>-clickhouse-headless). */}}
{{- define "ekokube.clickhouseHost" -}}
{{- if .Values.clickhouse.host -}}
{{ .Values.clickhouse.host }}
{{- else -}}
{{ .Release.Name }}-clickhouse-headless.{{ .Release.Namespace }}.svc
{{- end -}}
{{- end }}

{{- define "ekokube.clickhouseUrl" -}}
http://{{ include "ekokube.clickhouseHost" . }}:{{ .Values.clickhouse.httpPort }}
{{- end }}

{{/* Name of the Secret holding the plaintext ClickHouse password. */}}
{{- define "ekokube.authSecretName" -}}
{{- if .Values.clickhouse.auth.existingSecret -}}
{{ .Values.clickhouse.auth.existingSecret }}
{{- else -}}
{{ .Release.Name }}-clickhouse-auth
{{- end -}}
{{- end }}

{{/* sha256 hex of the ClickHouse password, for extraUsersConfig. */}}
{{- define "ekokube.passwordSha256" -}}
{{- if .Values.clickhouse.auth.existingSecret -}}
{{ required "clickhouse.auth.passwordSha256 is required when using existingSecret" .Values.clickhouse.auth.passwordSha256 }}
{{- else -}}
{{ sha256sum .Values.clickhouse.auth.password }}
{{- end -}}
{{- end }}

{{- define "ekokube.clickhouseEnv" -}}
- name: EKOKUBE_CLICKHOUSE_URL
  value: {{ include "ekokube.clickhouseUrl" . | quote }}
- name: EKOKUBE_CLICKHOUSE_DATABASE
  value: {{ .Values.clickhouse.database | quote }}
- name: EKOKUBE_CLICKHOUSE_USER
  value: {{ .Values.clickhouse.auth.username | quote }}
- name: EKOKUBE_CLICKHOUSE_PASSWORD
  valueFrom:
    secretKeyRef:
      name: {{ include "ekokube.authSecretName" . }}
      key: password
{{- end }}

{{/* Replica counts derived from the ha preset unless overridden. */}}
{{- define "ekokube.clickhouseReplicas" -}}
{{- if .Values.clickhouse.replicas -}}
{{ .Values.clickhouse.replicas }}
{{- else if .Values.clickhouse.ha -}}
2
{{- else -}}
1
{{- end -}}
{{- end }}

{{- define "ekokube.keeperReplicas" -}}
{{- if .Values.clickhouse.keeper.replicas -}}
{{ .Values.clickhouse.keeper.replicas }}
{{- else if .Values.clickhouse.ha -}}
3
{{- else -}}
1
{{- end -}}
{{- end }}
