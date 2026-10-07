"use client";
import { iconSizes } from "@/styles/tokens";

import { Combobox } from "@base-ui/react/combobox";
import { Dialog } from "@base-ui/react/dialog";
import { Field } from "@base-ui/react/field";
import { Form } from "@base-ui/react/form";
import { Toast } from "@base-ui/react/toast";
import {
  isCancelledError,
  useIsFetching,
  useMutation,
  useQueryClient,
} from "@tanstack/react-query";
import { Check, ChevronDown, RotateCcw, X } from "lucide-react";
import { Fragment, useRef, useState, useTransition } from "react";
import { Controller, useForm } from "react-hook-form";
import {
  memberMeQueryKey,
  nicknameAvailabilityOptions,
  nicknameSuggestionOptions,
  nicknameSuggestionQueryKey,
  publicProfileQueryKey,
  searchCompaniesOptions,
  searchCompaniesQueryKey,
  updateProfileMutation,
} from "@/api/generated/@tanstack/react-query.gen";
import { Button } from "@/components/button";
import { JobRoleDialog } from "./job-role-dialog";
import { jobRoleDialog } from "./job-role-dialog-handle";
import * as jobRoleStyles from "./job-role-field.css";
import { JobRolePill } from "./job-role-pill";
import type { MyPageData, ProfileCompany } from "./mypage-model";
import * as pillFieldStyles from "./profile-pill-field.css";
import * as panelStyles from "./mypage-panel.css";
import * as styles from "./profile-editor.css";

type ProfileFormValues = {
  bio: string;
  interestCompanies: ProfileCompany[];
  interestJobRoleIds: number[];
  nickname: string;
};

type ProfileEditorProps = Pick<MyPageData, "jobRoleGroups" | "member">;

type CompanyComboboxProps = {
  name: string;
  onBlur: () => void;
  onChange: (value: ProfileCompany[]) => void;
  value: ProfileCompany[];
};

function getDefaultValues(member: MyPageData["member"]): ProfileFormValues {
  return {
    bio: member.profile.bio,
    interestCompanies: member.profile.interestCompanies,
    interestJobRoleIds: member.profile.interestJobRoleIds,
    nickname: member.nickname,
  };
}

function getErrorMessage(error: unknown, fallback: string) {
  if (typeof error !== "object" || error === null || !("error" in error)) {
    return fallback;
  }

  const detail = error.error;

  if (typeof detail !== "object" || detail === null || !("message" in detail)) {
    return fallback;
  }

  return typeof detail.message === "string" ? detail.message : fallback;
}

function mergeCompanies(results: ProfileCompany[], selected: ProfileCompany[]) {
  const merged = [...results];

  selected.forEach((company) => {
    if (!merged.some((item) => item.companyId === company.companyId)) {
      merged.push(company);
    }
  });

  return merged;
}

function CompanyCombobox({ name, onBlur, onChange, value }: CompanyComboboxProps) {
  const queryClient = useQueryClient();
  const [searchValue, setSearchValue] = useState("");
  const [searchResults, setSearchResults] = useState<ProfileCompany[]>(value);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [blockStartStatus, setBlockStartStatus] = useState(false);
  const [isPending, startTransition] = useTransition();
  const selectedValuesRef = useRef(value);
  const query = searchValue.trim();
  const items = mergeCompanies(searchResults, value);

  const status = isPending
    ? "회사를 검색하고 있어요."
    : searchError ||
      (!query && !blockStartStatus && value.length === 0
        ? "회사명을 입력해 주세요."
        : searchResults.length === 0 && !blockStartStatus
          ? `"${query}" 검색 결과가 없어요.`
          : null);
  const emptyMessage =
    query && !isPending && !searchError && searchResults.length === 0
      ? "다른 회사명을 검색해 보세요."
      : null;

  return (
    <Combobox.Root<ProfileCompany, true>
      filter={null}
      inputValue={searchValue}
      isItemEqualToValue={(item, selected) => item.companyId === selected.companyId}
      itemToStringLabel={(company) => company.name}
      itemToStringValue={(company) => String(company.companyId)}
      items={items}
      multiple
      name={name}
      onInputValueChange={(nextSearchValue, details) => {
        setSearchValue(nextSearchValue);

        void queryClient.cancelQueries({ queryKey: searchCompaniesQueryKey() });
        const nextQuery = nextSearchValue.trim();

        if (!nextQuery) {
          setSearchResults(selectedValuesRef.current);
          setSearchError(null);
          setBlockStartStatus(false);
          return;
        }

        if (details.reason === "item-press") {
          return;
        }

        startTransition(async () => {
          setSearchError(null);

          try {
            const result = await queryClient.fetchQuery(
              searchCompaniesOptions({ query: { query: nextQuery } }),
            );

            startTransition(() => {
              setSearchResults(result.data?.companies ?? []);
              setSearchError(null);
            });
          } catch (error) {
            if (!isCancelledError(error)) {
              startTransition(() => {
                setSearchResults([]);
                setSearchError(
                  getErrorMessage(error, "회사 검색에 실패했어요. 다시 시도해 주세요."),
                );
              });
            }
          }
        });
      }}
      onOpenChangeComplete={(open) => {
        if (!open) {
          void queryClient.cancelQueries({ queryKey: searchCompaniesQueryKey() });
          setSearchValue("");
          setSearchResults(selectedValuesRef.current);
          setSearchError(null);
          setBlockStartStatus(false);
        }
      }}
      onValueChange={(nextValue) => {
        selectedValuesRef.current = nextValue;
        onChange(nextValue);
        setSearchValue("");
        setSearchError(null);

        if (nextValue.length === 0) {
          setSearchResults([]);
          setBlockStartStatus(false);
        } else {
          setBlockStartStatus(true);
        }
      }}
      value={value}
    >
      <Combobox.InputGroup className={`${pillFieldStyles.frame} ${styles.companyPillFrame}`}>
        <Combobox.Chips className={pillFieldStyles.pills}>
          <Combobox.Value>
            {(selectedCompanies: ProfileCompany[]) => (
              <Fragment>
                {selectedCompanies.map((company) => (
                  <Combobox.Chip
                    aria-label={company.name}
                    className={pillFieldStyles.pill}
                    key={company.companyId}
                  >
                    {company.name}
                    <Combobox.ChipRemove
                      aria-label={`${company.name} 삭제`}
                      className={pillFieldStyles.pillRemove}
                    >
                      <X aria-hidden="true" size={iconSizes.sm} strokeWidth={2} />
                    </Combobox.ChipRemove>
                  </Combobox.Chip>
                ))}
                <Combobox.Input
                  autoComplete="off"
                  className={styles.companyInput}
                  maxLength={50}
                  onBlur={onBlur}
                  placeholder={selectedCompanies.length > 0 ? "" : "회사를 검색해 추가해요"}
                  spellCheck={false}
                />
              </Fragment>
            )}
          </Combobox.Value>
        </Combobox.Chips>
      </Combobox.InputGroup>
      <Combobox.Portal>
        <Combobox.Positioner className={styles.positioner} sideOffset={4}>
          <Combobox.Popup aria-busy={isPending || undefined} className={styles.popup}>
            <Combobox.Status>
              {status ? <div className={styles.searchStatus}>{status}</div> : null}
            </Combobox.Status>
            <Combobox.Empty data-testid="company-search-empty">
              {emptyMessage ? <div className={styles.empty}>{emptyMessage}</div> : null}
            </Combobox.Empty>
            <Combobox.List className={styles.list}>
              {(company: ProfileCompany) => (
                <Combobox.Item className={styles.item} key={company.companyId} value={company}>
                  <Combobox.ItemIndicator className={styles.itemIndicator}>
                    <Check aria-hidden="true" size={iconSizes.sm} strokeWidth={2} />
                  </Combobox.ItemIndicator>
                  <span>{company.name}</span>
                </Combobox.Item>
              )}
            </Combobox.List>
          </Combobox.Popup>
        </Combobox.Positioner>
      </Combobox.Portal>
    </Combobox.Root>
  );
}

export function ProfileEditor({ jobRoleGroups, member }: ProfileEditorProps) {
  const queryClient = useQueryClient();
  const isNicknameSuggestionFetching =
    useIsFetching({ queryKey: nicknameSuggestionQueryKey(), exact: true }) > 0;
  const updateProfile = useMutation({
    ...updateProfileMutation(),
    onSuccess: () => {
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: memberMeQueryKey() }),
        queryClient.invalidateQueries({
          queryKey: publicProfileQueryKey({ path: { memberId: member.memberId } }),
        }),
      ]);
    },
  });
  const toastManager = Toast.useToastManager();
  const formValues = getDefaultValues(member);
  const {
    clearErrors,
    control,
    formState: { errors, isDirty },
    handleSubmit,
    reset,
    setError,
    setValue,
  } = useForm<ProfileFormValues>({
    mode: "onBlur",
    resetOptions: {
      keepDirtyValues: true,
      keepErrors: true,
      keepTouched: true,
    },
    values: formValues,
  });
  const suggestNickname = async () => {
    try {
      const result = await queryClient.fetchQuery(nicknameSuggestionOptions());
      const suggestedNickname = result.data?.nickname;

      if (!suggestedNickname) {
        throw new Error("Nickname suggestion response did not include a nickname.");
      }

      setValue("nickname", suggestedNickname, {
        shouldDirty: true,
        shouldTouch: true,
        shouldValidate: true,
      });
    } catch {
      setError("nickname", {
        message: "새 닉네임을 만들지 못했어요. 잠시 후 다시 시도해 주세요.",
        type: "suggestion",
      });
    }
  };
  const submitError = errors.root?.serverError?.message;

  const submitForm = handleSubmit((values) => {
    const normalizedValues = { ...values, nickname: values.nickname.trim() };

    if (!isDirty) {
      return;
    }

    updateProfile.mutate(
      {
        body: {
          nickname: normalizedValues.nickname,
          bio: normalizedValues.bio,
          interestCompanyIds: normalizedValues.interestCompanies.map(
            (company) => company.companyId,
          ),
          interestJobRoleIds: normalizedValues.interestJobRoleIds,
        },
      },
      {
        onError: (error) => {
          if (error.error?.code === "E1007") {
            setError("nickname", {
              message: "이미 사용 중인 닉네임이에요.",
              type: "server",
            });
            return;
          }

          setError("root.serverError", {
            message: getErrorMessage(error, "프로필을 저장하지 못했어요."),
            type: "server",
          });
        },
        onSuccess: () => {
          reset(normalizedValues);
          toastManager.add({ title: "프로필을 저장했어요." });
        },
      },
    );
  });

  return (
    <Form className={styles.form} onSubmit={submitForm}>
      <div className={panelStyles.card}>
        <h2 className={panelStyles.title}>프로필 수정</h2>
        <div className={styles.firstRow}>
          <Controller
            control={control}
            name="nickname"
            rules={{
              validate: async (value) => {
                const nickname = value.trim();

                if (!nickname) {
                  return "닉네임을 입력해 주세요.";
                }

                if (nickname === member.nickname) {
                  return true;
                }

                try {
                  const result = await queryClient.fetchQuery(
                    nicknameAvailabilityOptions({ query: { nickname } }),
                  );

                  return result.data?.available || "이미 사용 중인 닉네임이에요.";
                } catch (error) {
                  return getErrorMessage(error, "닉네임을 확인하지 못했어요.");
                }
              },
            }}
            render={({ field, fieldState }) => (
              <Field.Root
                className={styles.field}
                dirty={fieldState.isDirty}
                invalid={fieldState.invalid}
                name={field.name}
                touched={fieldState.isTouched}
              >
                <Field.Label className={styles.label}>닉네임</Field.Label>
                <div className={styles.nicknameInputGroup}>
                  <Field.Control
                    className={styles.nicknameInput}
                    onBlur={field.onBlur}
                    onValueChange={(nextValue) => {
                      field.onChange(nextValue);
                      clearErrors("nickname");
                      clearErrors("root.serverError");
                    }}
                    required
                    value={field.value}
                  />
                  <Button
                    className={styles.suggestionButton}
                    disabled={isNicknameSuggestionFetching}
                    onClick={() => void suggestNickname()}
                    size="sm"
                    type="button"
                    variant="ghost"
                  >
                    <RotateCcw aria-hidden="true" size={iconSizes.sm} strokeWidth={2} />
                    {isNicknameSuggestionFetching ? "만드는 중..." : "새로 만들기"}
                  </Button>
                </div>
                <Field.Error
                  className={`${styles.fieldMessage} ${styles.errorMessage}`}
                  match={Boolean(fieldState.error)}
                >
                  {fieldState.error?.message}
                </Field.Error>
              </Field.Root>
            )}
          />

          <Controller
            control={control}
            name="interestJobRoleIds"
            render={({ field, fieldState }) => {
              const selectedJobRoles = jobRoleGroups
                .flatMap((group) => group.roles)
                .filter((role) => field.value.includes(role.jobRoleId));
              const handleValueChange = (nextValue: number[]) => {
                field.onChange(nextValue);
                clearErrors("root.serverError");
              };

              return (
                <Fragment>
                  <Field.Root
                    className={styles.field}
                    dirty={fieldState.isDirty}
                    invalid={fieldState.invalid}
                    name={field.name}
                    touched={fieldState.isTouched}
                  >
                    <Field.Label className={styles.label}>관심 직무</Field.Label>
                    <div className={jobRoleStyles.fieldFrame}>
                      <Field.Control
                        className={jobRoleStyles.fieldTrigger}
                        onBlur={field.onBlur}
                        ref={field.ref}
                        render={<Dialog.Trigger handle={jobRoleDialog} />}
                        type="button"
                        value={field.value.join(",")}
                      />
                      <div className={jobRoleStyles.fieldContent}>
                        {selectedJobRoles.length > 0 ? (
                          selectedJobRoles.map((role) => (
                            <JobRolePill
                              key={role.jobRoleId}
                              onRemove={() =>
                                handleValueChange(field.value.filter((id) => id !== role.jobRoleId))
                              }
                              role={role}
                              variant="field"
                            />
                          ))
                        ) : (
                          <span className={pillFieldStyles.placeholder}>
                            관심 직무를 선택해 주세요.
                          </span>
                        )}
                        <ChevronDown
                          aria-hidden="true"
                          className={jobRoleStyles.fieldChevron}
                          size={iconSizes.sm}
                        />
                      </div>
                    </div>
                  </Field.Root>
                  <JobRoleDialog
                    groups={jobRoleGroups}
                    mode="multiple"
                    onValueChange={handleValueChange}
                    value={field.value}
                  />
                </Fragment>
              );
            }}
          />
        </div>

        <Controller
          control={control}
          name="bio"
          render={({ field, fieldState }) => (
            <Field.Root
              className={styles.field}
              dirty={fieldState.isDirty}
              invalid={fieldState.invalid}
              name={field.name}
              touched={fieldState.isTouched}
            >
              <Field.Label className={styles.label}>자기소개</Field.Label>
              <Field.Control
                className={styles.bioInput}
                maxLength={500}
                onBlur={field.onBlur}
                onValueChange={(nextValue) => {
                  field.onChange(nextValue);
                  clearErrors("root.serverError");
                }}
                render={<textarea rows={2} />}
                value={field.value}
              />
            </Field.Root>
          )}
        />

        <Controller
          control={control}
          name="interestCompanies"
          render={({ field, fieldState }) => (
            <Field.Root
              className={styles.field}
              dirty={fieldState.isDirty}
              invalid={fieldState.invalid}
              name={field.name}
              touched={fieldState.isTouched}
            >
              <Field.Label className={styles.label}>관심 회사</Field.Label>
              <CompanyCombobox
                name={field.name}
                onBlur={field.onBlur}
                onChange={(nextValue) => {
                  field.onChange(nextValue);
                  clearErrors("root.serverError");
                }}
                value={field.value}
              />
            </Field.Root>
          )}
        />

        <div className={panelStyles.footer}>
          {submitError && (
            <p className={styles.submitError} role="alert">
              {submitError}
            </p>
          )}
          <Button className={panelStyles.action} type="submit">
            저장하기
          </Button>
        </div>
      </div>
    </Form>
  );
}
