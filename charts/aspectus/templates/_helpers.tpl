{{/*
Expand the name of the chart.
*/}}
{{- define "aspectus.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "aspectus.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "aspectus.labels" -}}
helm.sh/chart: {{ include "aspectus.name" . }}-{{ .Chart.Version | replace "+" "_" }}
app.kubernetes.io/name: aspectus
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: pandaria
{{- end }}

{{/*
Selector labels
*/}}
{{- define "aspectus.selectorLabels" -}}
app.kubernetes.io/name: aspectus
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}
