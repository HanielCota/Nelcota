// Server error codes (admin API `code` field) → text in each language.
// Keys must match the codes in crates/admin/src; placeholders match `params`.
// Errors without a code (e.g. Postgres messages) are shown as the server sent them.
import type { Messages } from '../index.svelte'

const ptBR = {
  // Session and access
  route_not_found: 'Rota não encontrada.',
  session_expired: 'Sessão expirada: entre de novo.',
  origin_not_allowed: 'Origem não permitida.',
  too_many_attempts: 'Muitas tentativas. Aguarde um minuto.',
  invalid_credentials: 'Email ou senha inválidos.',
  sso_disabled: 'Login único desligado: entre no painel de cada projeto.',
  project_not_found: "O projeto '{project}' não existe neste host.",
  sso_link_invalid: 'Link de acesso inválido ou expirado: entre de novo.',

  // Tables and rows
  table_not_found: "A tabela '{table}' não existe no schema exposto.",
  invalid_filters: 'Filtros inválidos: {detail}',
  invalid_query: 'Consulta inválida: {detail}',
  unknown_or_generated_column: 'Coluna desconhecida ou gerada: {column}.',
  invalid_json_value: '{column}: JSON inválido ({detail}).',
  no_primary_key: 'Tabela sem chave primária: edição pelo painel indisponível (use o editor SQL).',
  missing_key_value: "Valor da chave '{column}' ausente.",
  no_rows_selected: 'Nenhuma linha selecionada.',

  // Users
  invalid_id: 'Id inválido.',
  user_not_found: 'Usuário não encontrado.',
  user_already_exists: 'Já existe um usuário com este email.',
  invalid_email: 'Email inválido.',
  password_too_short: 'A senha precisa de pelo menos {min} caracteres.',
  password_too_long: 'A senha pode ter no máximo {max} caracteres.',

  // Tokens and profile
  invalid_token_validity: 'Validade entre 1 e {max} dias.',
  no_profile_photo: 'Sem foto de perfil.',
  invalid_image_base64: 'Imagem inválida (base64).',
  photo_too_large: 'A foto deve ter até {max_kb} KB.',
  unsupported_image_type: 'Use uma imagem PNG, JPEG ou WebP.',

  // Migrations
  invalid_migration_name: 'Nome com letras minúsculas, números e _ (até 60), ex.: criar_pedidos.',
  no_pending_changes: 'Nenhuma alteração do painel fora das migrações.',
  migration_not_from_panel: 'Esta migração não foi gerada pelo painel.',

  // Structure (DDL) validation
  name_empty: 'Nome vazio.',
  name_surrounding_spaces: "'{name}': sem espaços no começo ou no fim.",
  name_too_long: "'{name}': máximo de {max} bytes.",
  name_invalid_character: 'Nome com caractere inválido.',
  expression_empty: '{kind}: expressão vazia.',
  type_unclosed_parenthesis: "Tipo '{type}': parêntese sem fechar.",
  unknown_type: "Tipo desconhecido: '{type}'.",
  invalid_type_modifier: "Tipo '{type}': modificador inválido.",
  identity_requires_integer: "Coluna '{column}': identity só em smallint, integer ou bigint.",
  identity_with_default: "Coluna '{column}': identity não aceita DEFAULT.",
  table_needs_columns: 'A tabela precisa de pelo menos uma coluna.',
  duplicate_column: "Coluna '{column}' repetida.",
  no_changes: 'Nenhuma alteração.',
  primary_key_on_create_only: 'A chave primária é definida na criação da tabela.',
  column_already_exists: "A coluna '{column}' já existe.",
  column_not_found: "A coluna '{column}' não existe.",
  column_rejects_default: "A coluna '{column}' é identity/gerada: não aceita DEFAULT.",
  policy_insert_no_using: 'Policy de INSERT não usa USING: a linha ainda não existe (use WITH CHECK).',
  policy_check_not_allowed: 'Policy de {command} não usa WITH CHECK: nenhuma linha é gravada (use USING).',
  policy_insert_needs_check: 'Policy de INSERT precisa de WITH CHECK.',
  policy_needs_using: 'A policy precisa da expressão USING.',
}

const en: Messages<typeof ptBR> = {
  route_not_found: 'Route not found.',
  session_expired: 'Session expired: sign in again.',
  origin_not_allowed: 'Origin not allowed.',
  too_many_attempts: 'Too many attempts. Wait a minute.',
  invalid_credentials: 'Invalid email or password.',
  sso_disabled: "Single sign-on is off: sign in to each project's panel.",
  project_not_found: "Project '{project}' does not exist on this host.",
  sso_link_invalid: 'Invalid or expired access link: sign in again.',

  table_not_found: "Table '{table}' does not exist in the exposed schema.",
  invalid_filters: 'Invalid filters: {detail}',
  invalid_query: 'Invalid query: {detail}',
  unknown_or_generated_column: 'Unknown or generated column: {column}.',
  invalid_json_value: '{column}: invalid JSON ({detail}).',
  no_primary_key: 'Table without a primary key: editing from the panel is unavailable (use the SQL editor).',
  missing_key_value: "Missing value for key '{column}'.",
  no_rows_selected: 'No rows selected.',

  invalid_id: 'Invalid id.',
  user_not_found: 'User not found.',
  user_already_exists: 'A user with this email already exists.',
  invalid_email: 'Invalid email.',
  password_too_short: 'The password needs at least {min} characters.',
  password_too_long: 'The password can have at most {max} characters.',

  invalid_token_validity: 'Validity between 1 and {max} days.',
  no_profile_photo: 'No profile photo.',
  invalid_image_base64: 'Invalid image (base64).',
  photo_too_large: 'The photo must be at most {max_kb} KB.',
  unsupported_image_type: 'Use a PNG, JPEG or WebP image.',

  invalid_migration_name: 'Name with lowercase letters, digits and _ (up to 60), e.g. create_orders.',
  no_pending_changes: 'No panel changes outside migrations.',
  migration_not_from_panel: 'This migration was not generated by the panel.',

  name_empty: 'Empty name.',
  name_surrounding_spaces: "'{name}': no leading or trailing spaces.",
  name_too_long: "'{name}': at most {max} bytes.",
  name_invalid_character: 'Name with an invalid character.',
  expression_empty: '{kind}: empty expression.',
  type_unclosed_parenthesis: "Type '{type}': unclosed parenthesis.",
  unknown_type: "Unknown type: '{type}'.",
  invalid_type_modifier: "Type '{type}': invalid modifier.",
  identity_requires_integer: "Column '{column}': identity only on smallint, integer or bigint.",
  identity_with_default: "Column '{column}': identity does not take a DEFAULT.",
  table_needs_columns: 'The table needs at least one column.',
  duplicate_column: "Column '{column}' repeated.",
  no_changes: 'No changes.',
  primary_key_on_create_only: 'The primary key is set when the table is created.',
  column_already_exists: "Column '{column}' already exists.",
  column_not_found: "Column '{column}' does not exist.",
  column_rejects_default: "Column '{column}' is identity/generated: it does not take a DEFAULT.",
  policy_insert_no_using: 'An INSERT policy does not use USING: the row does not exist yet (use WITH CHECK).',
  policy_check_not_allowed: 'A {command} policy does not use WITH CHECK: no row is written (use USING).',
  policy_insert_needs_check: 'An INSERT policy needs WITH CHECK.',
  policy_needs_using: 'The policy needs a USING expression.',
}

export default { 'pt-BR': ptBR, en }
