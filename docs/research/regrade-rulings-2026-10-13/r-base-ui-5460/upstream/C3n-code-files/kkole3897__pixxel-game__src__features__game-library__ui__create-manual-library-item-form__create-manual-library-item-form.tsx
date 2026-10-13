'use client';

import { Controller } from 'react-hook-form';
import cn from 'classnames';
import { RiDeleteBinLine, RiAddCircleLine } from '@remixicon/react';
import { v4 as uuidv4 } from 'uuid';

import * as LibraryField from '../library-field';
import { PlayRecordFieldset } from '../play-record-fieldset';
import { UploadMainImageInput } from '../upload-main-image-input';
import {
  usePlayRecordsFieldArray,
  useCreateManualLibraryItemForm,
  formatPlayStatus,
  type CreateValidManualLibraryItemFormValues,
} from '../../lib';
import { type PlayStatus } from '../../model';
import { PLAY_STATUS } from '../../constants';
import { type GameDrm, GAME_DRM, formatDrm } from '@/entities/game';
import * as Input from '@/shared/ui/input';
import { Select } from '@/shared/ui/select';
import { Checkbox } from '@/shared/ui/checkbox';
import { Textarea } from '@/shared/ui/textarea';
import { Button } from '@/shared/ui/button';
import * as styles from './create-manual-library-item-form.css';

type CreateManualLibraryItemFormProps = {
  onSubmit?: (data: CreateValidManualLibraryItemFormValues) => void;
};

export default function CreateManualLibraryItemForm({
  onSubmit,
}: CreateManualLibraryItemFormProps) {
  const {
    control,
    watch,
    register,
    formState: { errors },
    setValue,
    clearErrors,
    createHandleSubmit,
  } = useCreateManualLibraryItemForm({
    defaultValues: {
      title: '',
      mainImage: null,
      publicId: uuidv4(),
      playRecords: [
        {
          drm: null,
          isCustomDrm: false,
          playStatus: null,
          isCleared: false,
          playTime: 0,
          memo: null,
        },
      ],
    },
  });
  const { fields, remove, append } = usePlayRecordsFieldArray({ control });

  const playStatusItems = Object.values(PLAY_STATUS);

  const handleSubmit = createHandleSubmit((data) => {
    console.log(data);
    onSubmit?.(data);
  });

  return (
    <form onSubmit={handleSubmit}>
      <div>
        <LibraryField.Root name="title">
          <LibraryField.Label>
            타이틀 <LibraryField.RequiredIndicator />
          </LibraryField.Label>
          <LibraryField.Control>
            <Input.Root {...register('title', { required: true })} />
          </LibraryField.Control>
        </LibraryField.Root>
        <LibraryField.Root name="mainImage" className={styles.mainImageField}>
          <LibraryField.Label>메인 이미지</LibraryField.Label>
          <LibraryField.Control>
            <Controller
              control={control}
              name="mainImage"
              render={({ field: { name, value } }) => (
                <UploadMainImageInput
                  url={value}
                  name={name}
                  onUpload={(file) => {
                    if (file) {
                      setValue(name, URL.createObjectURL(file));
                    }
                  }}
                  onDelete={() => {
                    setValue(name, null);
                  }}
                />
              )}
            />
          </LibraryField.Control>
          <LibraryField.HelperText>
            * 최대 용량: 50MB | 권장 사이즈: 720 X 540
          </LibraryField.HelperText>
        </LibraryField.Root>
        {fields.map((field, index) => (
          <PlayRecordFieldset.Root key={field.id}>
            <PlayRecordFieldset.Header>
              <PlayRecordFieldset.Legend>
                플레이 {index + 1}
              </PlayRecordFieldset.Legend>
            </PlayRecordFieldset.Header>
            <PlayRecordFieldset.Content>
              <LibraryField.Root name={`playRecords.${index}.drm`}>
                <LibraryField.Label>
                  DRM <LibraryField.RequiredIndicator />
                </LibraryField.Label>
                {watch(`playRecords.${index}.isCustomDrm`) ? (
                  <>
                    <LibraryField.Control>
                      <Input.Root
                        {...register(`playRecords.${index}.drm`, {
                          required: true,
                          minLength: 1,
                        })}
                      />
                    </LibraryField.Control>
                    {errors.playRecords?.[index]?.drm && (
                      <LibraryField.ErrorText>
                        필수 입력 항목입니다.
                      </LibraryField.ErrorText>
                    )}
                  </>
                ) : (
                  <>
                    <Controller
                      control={control}
                      name={`playRecords.${index}.drm`}
                      rules={{ required: true }}
                      render={({ field: { value, name } }) => (
                        <Select.Root
                          name={name}
                          value={value ?? ''}
                          required
                          onValueChange={(value) => {
                            setValue(name, value as GameDrm);
                            clearErrors(name);
                          }}
                        >
                          <LibraryField.Control>
                            <Select.Trigger className={styles.selectTrigger}>
                              <Select.Value />
                              <Select.Icon />
                            </Select.Trigger>
                          </LibraryField.Control>
                          <Select.Content sideOffset={8}>
                            {Object.values(GAME_DRM).map((drm) => (
                              <Select.Item key={drm} value={drm}>
                                {formatDrm(drm)}
                              </Select.Item>
                            ))}
                          </Select.Content>
                        </Select.Root>
                      )}
                    />
                    {errors.playRecords?.[index]?.drm && (
                      <LibraryField.ErrorText>
                        필수 선택 항목입니다.
                      </LibraryField.ErrorText>
                    )}
                  </>
                )}
                <LibraryField.Root
                  name={`playRecords.${index}.isCustomDrm`}
                  className={cn(styles.subField, styles.checkboxGroup)}
                >
                  <Controller
                    control={control}
                    name={`playRecords.${index}.isCustomDrm`}
                    render={({ field: { value, name } }) => (
                      <LibraryField.Control className={styles.checkboxControl}>
                        <Checkbox
                          name={name}
                          value="true"
                          checked={value}
                          onCheckedChange={(isChecked) => {
                            if (isChecked === true) {
                              setValue(name, isChecked);
                              setValue(`playRecords.${index}.drm`, '');
                            } else if (isChecked === false) {
                              setValue(name, isChecked);
                              setValue(`playRecords.${index}.drm`, null);
                            }
                          }}
                        />
                      </LibraryField.Control>
                    )}
                  />
                  <LibraryField.Label className={styles.checkboxLabel}>
                    직접 입력하기
                  </LibraryField.Label>
                </LibraryField.Root>
              </LibraryField.Root>
              <LibraryField.Root name={`playRecords.${index}.playTime`}>
                <LibraryField.Label>플레이 시간</LibraryField.Label>
                <LibraryField.Control>
                  <Input.Root
                    {...register(`playRecords.${index}.playTime`, {
                      min: 0,
                      valueAsNumber: true,
                      onBlur: (event) => {
                        if (event.target.value === '') {
                          setValue(`playRecords.${index}.playTime`, 0);
                        }
                      },
                    })}
                    type="number"
                    step="0.1"
                  >
                    <Input.Slot side="right">시간</Input.Slot>
                  </Input.Root>
                </LibraryField.Control>
                {errors.playRecords?.[index]?.playTime && (
                  <LibraryField.ErrorText>
                    0보다 크거나 같아야 합니다.
                  </LibraryField.ErrorText>
                )}
              </LibraryField.Root>
              <LibraryField.Root name={`playRecords.${index}.playStatus`}>
                <LibraryField.Label>플레이 상태</LibraryField.Label>
                <Controller
                  control={control}
                  name={`playRecords.${index}.playStatus`}
                  render={({ field: { name, value } }) => (
                    <Select.Root
                      name={name}
                      value={value ?? ''}
                      onValueChange={(value) => {
                        setValue(name, value as PlayStatus);
                      }}
                    >
                      <LibraryField.Control>
                        <Select.Trigger className={styles.selectTrigger}>
                          <Select.Value />
                          <Select.Icon />
                        </Select.Trigger>
                      </LibraryField.Control>
                      <Select.Content>
                        {playStatusItems.map((playStatus) => (
                          <Select.Item key={playStatus} value={playStatus}>
                            {formatPlayStatus(playStatus)}
                          </Select.Item>
                        ))}
                      </Select.Content>
                    </Select.Root>
                  )}
                />
                <LibraryField.Root
                  name={`playRecords.${index}.isCleared`}
                  className={cn(styles.subField, styles.checkboxGroup)}
                >
                  <Controller
                    control={control}
                    name={`playRecords.${index}.isCleared`}
                    render={({ field: { value } }) => (
                      <LibraryField.Control className={styles.checkboxControl}>
                        <Checkbox
                          value="true"
                          checked={value}
                          onCheckedChange={(isChecked) => {
                            if (isChecked === 'indeterminate') {
                              return;
                            }

                            setValue(
                              `playRecords.${index}.isCleared`,
                              isChecked
                            );
                          }}
                        />
                      </LibraryField.Control>
                    )}
                  />
                  <LibraryField.Label className={styles.checkboxLabel}>
                    클리어
                  </LibraryField.Label>
                </LibraryField.Root>
              </LibraryField.Root>
              <LibraryField.Root name={`playRecords.${index}.memo`}>
                <LibraryField.Label>메모</LibraryField.Label>
                <LibraryField.Control>
                  <Textarea {...register(`playRecords.${index}.memo`)} />
                </LibraryField.Control>
              </LibraryField.Root>
            </PlayRecordFieldset.Content>
            <PlayRecordFieldset.Footer>
              {fields.length > 1 && (
                <Button
                  type="button"
                  variant="text"
                  className={styles.deletePlayRecordButton}
                  aria-label={`플레이 ${index + 1} 삭제`}
                  onClick={() => remove(index)}
                >
                  <RiDeleteBinLine size={20} />
                </Button>
              )}
            </PlayRecordFieldset.Footer>
          </PlayRecordFieldset.Root>
        ))}
        <div className={styles.actionArea}>
          <Button
            type="button"
            variant="ghost"
            onClick={() => {
              append({
                drm: null,
                isCustomDrm: false,
                playTime: 0,
                isCleared: false,
                playStatus: null,
                memo: null,
              });
            }}
          >
            <RiAddCircleLine
              size={20}
              className={styles.addPlayRecordButtonIcon}
            />
            플레이 정보 추가
          </Button>
          <Button type="submit">등록하기</Button>
        </div>
      </div>
    </form>
  );
}
